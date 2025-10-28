use bip39::{Mnemonic, Seed};
use mutrade_config::MutradeConfig;
use serde::Serialize;
use sol_token_type::SolTokenType;
use solana_account_decoder_client_types::{ParsedAccount, UiAccountData};
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_response::RpcKeyedAccount};
use solana_keypair::Keypair;
use solana_sdk::{
    derivation_path::DerivationPath,
    instruction::Instruction,
    message::{AddressLookupTableAccount, VersionedMessage, v0},
    program_error::ProgramError,
    pubkey::Pubkey,
    signer::SeedDerivable,
    transaction::VersionedTransaction,
};
use solana_signer::Signer;
use spl_associated_token_account::{
    get_associated_token_address_with_program_id,
    instruction::create_associated_token_account_idempotent,
};

pub use spl_token;
use tracing::instrument;

pub mod sol_token_type;
pub struct SolMut;
#[derive(Debug)]
pub enum SolError {
    OwnerNotToken,
    ErrorCompilingIx,
}
pub mod geyser;
pub mod sol_events;

impl SolMut {
    pub fn get_solana_client() -> RpcClient {
        let rpc_url = MutradeConfig::get_mutrade_config()
            .solana_rpcs
            .first()
            .cloned()
            .unwrap_or("https://api.mainnet-beta.solana.com".to_owned());
        let client = solana_client::nonblocking::rpc_client::RpcClient::new(rpc_url);
        client
    }
    pub fn build_versioned_transaction(
        instructions: Vec<Instruction>,
        payer: &Pubkey,
        recent_blockhash: solana_sdk::hash::Hash,
        keypair: &[&solana_keypair::Keypair],
        address_lookup_table: Vec<AddressLookupTableAccount>,
    ) -> Result<VersionedTransaction, SolError> {
        let v0_messgae = v0::Message::try_compile(
            payer,
            &instructions,
            &address_lookup_table,
            recent_blockhash,
        )
        .map_err(|x| {
            eprintln!("{x:?}");
            SolError::ErrorCompilingIx
        })?;
        let message = VersionedMessage::V0(v0_messgae);

        Ok(
            VersionedTransaction::try_new(message, keypair).map_err(|x| {
                eprintln!("{x:?}");
                SolError::ErrorCompilingIx
            })?,
        )
    }

    pub fn get_address_from_mnemonic(mnemonic: Mnemonic) -> Result<Keypair, ()> {
        let seed = Seed::new(&mnemonic, "");
        let keypair = Keypair::from_seed_and_derivation_path(
            seed.as_bytes(),
            Some(DerivationPath::from_absolute_path_str("m/44/501/0/0").map_err(|_| ())?),
        )
        .map_err(|_| ())?;
        Ok(keypair)
    }
    pub fn get_pda(
        wallet_address: &Pubkey,
        token_mint_address: &Pubkey,
        sol_token_type: SolTokenType,
    ) -> Pubkey {
        match sol_token_type {
            SolTokenType::SplToken => get_associated_token_address_with_program_id(
                wallet_address,
                token_mint_address,
                &spl_token::ID,
            ),
            SolTokenType::SplToken2022 => get_associated_token_address_with_program_id(
                wallet_address,
                token_mint_address,
                &spl_token_2022::ID,
            ),
        }
    }
    pub fn ix_transfer_tokens(
        sender: &spl_token::solana_program::pubkey::Pubkey,
        recipient: &spl_token::solana_program::pubkey::Pubkey,
        mint: &spl_token::solana_program::pubkey::Pubkey,
        amount: u64,
        decimals: u8,
        sol_token_type: Option<SolTokenType>,
    ) -> Result<Instruction, ProgramError> {
        let sol_token_type = match sol_token_type {
            Some(x) => x,
            None => todo!(),
        };
        let ix = match sol_token_type {
            SolTokenType::SplToken => {
                let program_id = spl_token::ID;
                let token_account_recv =
                    get_associated_token_address_with_program_id(recipient, mint, &program_id);
                let token_account_sender =
                    get_associated_token_address_with_program_id(sender, mint, &program_id);
                spl_token::instruction::transfer(
                    &spl_token::ID,
                    &token_account_sender,
                    &token_account_recv,
                    &sender,
                    &[&sender],
                    amount,
                )?
            }
            SolTokenType::SplToken2022 => {
                let program_id = spl_token_2022::ID;
                let token_account_recv =
                    get_associated_token_address_with_program_id(recipient, mint, &program_id);
                let token_account_sender =
                    get_associated_token_address_with_program_id(sender, mint, &program_id);
                spl_token_2022::instruction::transfer_checked(
                    &program_id,
                    &token_account_sender,
                    mint,
                    &token_account_recv,
                    sender,
                    &[sender],
                    amount,
                    decimals,
                )?
            }
        };
        Ok(ix)
    }

    pub fn create_ata(user: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Instruction {
        create_associated_token_account_idempotent(&user, &user, &mint, &token_program)
    }
}

#[derive(Debug, Serialize)]
pub struct AccountBalance {
    pub lamports: u64,
    pub token_accounts: Vec<ParsedAccount>,
}
#[instrument(skip(client))]
pub async fn get_solana_balances(
    client: &RpcClient,
    pubkey: &Pubkey,
) -> Result<AccountBalance, ()> {
    let balance_sol = client.get_balance(pubkey);
    let spl2022_balance = client.get_token_accounts_by_owner(
        pubkey,
        solana_client::rpc_request::TokenAccountsFilter::ProgramId(spl_token::id()),
    );
    let spl_balance = client.get_token_accounts_by_owner(
        pubkey,
        solana_client::rpc_request::TokenAccountsFilter::ProgramId(spl_token_2022::id()),
    );
    let (balance_sol, spl_balance, spl_2022_balance) =
        futures::join!(balance_sol, spl_balance, spl2022_balance);
    let balance_sol = balance_sol.map_err(|x| ())?;
    let mut spl_balance = spl_balance.map_err(|x| ())?;
    let spl2022_balance = spl_2022_balance.map_err(|x| ())?;

    spl_balance.extend(spl2022_balance);
    let mut to_return = vec![];
    for account in spl_balance {
        let data = account.account.data;
        if let UiAccountData::Json(x) = data {
            to_return.push(x);
        }
    }
    Ok(AccountBalance {
        lamports: balance_sol,
        token_accounts: to_return,
    })
}
