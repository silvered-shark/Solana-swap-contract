use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use solana_sdk::pubkey::Pubkey;

pub(crate) const ORCA_ADDRESS: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Default)]
pub struct SwapArgs {
    pub amount: u64,
    pub other_amount_threshold: u64,
    pub sqrt_price_limit: u128,
    pub amount_specified_is_input: bool,
    pub a_to_b: bool,
}

#[derive(Clone, Debug)]
pub struct SwapAccounts {
    pub whirlpool_program: Pubkey,    
    pub whirlpool: Pubkey,            
    pub token_program: Pubkey,        
    pub token_authority: Pubkey,      
    pub token_owner_account_a: Pubkey,
    pub token_vault_a: Pubkey,        
    pub token_owner_account_b: Pubkey,
    pub token_vault_b: Pubkey,        
    pub tick_array_0: Pubkey,         
    pub tick_array_1: Pubkey,         
    pub tick_array_2: Pubkey,         
    pub oracle: Pubkey,               
}

impl SwapAccounts {
    pub fn with_default_program(
        whirlpool: Pubkey,
        token_program: Pubkey,
        token_authority: Pubkey,
        token_owner_account_a: Pubkey,
        token_vault_a: Pubkey,
        token_owner_account_b: Pubkey,
        token_vault_b: Pubkey,
        tick_array_0: Pubkey,
        tick_array_1: Pubkey,
        tick_array_2: Pubkey,
        oracle: Pubkey,
    ) -> Self {
        Self {
            whirlpool_program: whirlpools_program_id(),
            whirlpool,
            token_program,
            token_authority,
            token_owner_account_a,
            token_vault_a,
            token_owner_account_b,
            token_vault_b,
            tick_array_0,
            tick_array_1,
            tick_array_2,
            oracle,
        }
    }
}

pub fn build_whirlpool_swap_ix(
    accts: &SwapAccounts,
    args: &SwapArgs,
) -> solana_sdk::instruction::Instruction {
    use solana_sdk::instruction::{AccountMeta, Instruction};

    let mut data = anchor_sighash_global_swap().to_vec();
    data.extend(borsh::to_vec(args).expect("borsh serialize SwapArgs"));

    let metas = vec![
        AccountMeta::new(accts.whirlpool, true).with_is_signer(false),
        AccountMeta::new_readonly(accts.token_program, false),
        AccountMeta::new_readonly(accts.token_authority, true),
        AccountMeta::new(accts.token_owner_account_a, false),
        AccountMeta::new(accts.token_vault_a, false),
        AccountMeta::new(accts.token_owner_account_b, false),
        AccountMeta::new(accts.token_vault_b, false),
        AccountMeta::new(accts.tick_array_0, false),
        AccountMeta::new(accts.tick_array_1, false),
        AccountMeta::new(accts.tick_array_2, false),
        AccountMeta::new_readonly(accts.oracle, false),
    ];

    Instruction {
        program_id: accts.whirlpool_program,
        accounts: metas,
        data,
    }
}

pub fn whirlpools_program_id() -> Pubkey {
    ::std::str::FromStr::from_str(ORCA_ADDRESS).expect("valid ORCA program id")
}

fn anchor_sighash_global_swap() -> [u8; 8] {
    let preimage = b"global:swap";
    let digest = solana_sdk::hash::hash(preimage);
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest.to_bytes()[0..8]);
    out
}

#[serde_as]
#[derive(BorshSerialize, BorshDeserialize, Clone, Serialize, Debug, Deserialize)]
pub struct Traded {
    #[borsh(skip)]
    pub signature: String,
    #[serde_as(as = "DisplayFromStr")]
    pub whirlpool: Pubkey,
    pub a_to_b: bool,
    pub pre_sqrt_price: u128,
    pub post_sqrt_price: u128,
    pub input_amount: u64,
    pub output_amount: u64,
    pub input_transfer_fee: u64,
    pub output_transfer_fee: u64,
    pub lp_fee: u64,
    pub protocol_fee: u64,
}
