use std::str::FromStr;

use crate::sol::SolMut;
use base64::{Engine, prelude::BASE64_STANDARD};
use mutrade_config::MutradeConfig;
use serde::Deserialize;
use serde_json::{Value, json};
use solana_program::system_instruction;
use solana_sdk::{
    address_lookup_table::state::AddressLookupTable,
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    message::AddressLookupTableAccount,
    pubkey::Pubkey,
};
use tracing::{error, info, instrument};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartiallyTypedQuote {
    pub input_mint: String,
    pub output_mint: String,
    pub out_amount: String,
    pub in_amount: String,
}
#[derive(Debug)]
pub enum JupiterError {
    ErrorBuildingSwap,
    ErrorGettingVersionedTransaction,
    ErrorDeserializingTransaction,
    GotNone,
    ErrorBuildingKey,
    ErrorGettingQuote,
    GotNoInAmount,
    ErrorParsingInAmount,
    GotNoOutAmount,
    ErrorParsingAmount,
    GotNoOutputMint,
    NetworkError,
    ErrorParsingQuote,
    NoInstructionRecieved,
    LookupAddressMissing,
    ErrorParsingPubkey,
    ErrorGettingData,
    ErrorDeserializingData,
}
#[instrument]
pub async fn get_jupiter_instruction(
    // input: JupiterGetQuoteInput,
    user: &Pubkey,
    quote: Value,
    with_priority: Option<u64>,
) -> Result<(Vec<Instruction>, Vec<AddressLookupTableAccount>), JupiterError> {
    let client = reqwest::Client::new();
    let sol_client = SolMut::get_solana_client();
    let swap_api = "https://lite-api.jup.ag/swap/v1/swap-instructions";
    let mut instructions = vec![];
    if let Some(priority_fee) = with_priority {
        let ix = ComputeBudgetInstruction::set_compute_unit_price(priority_fee);
        instructions.push(ix);
        info!("Priority fee instructions set");
    }

    let swap_json = json!({
        "quoteResponse": quote,
        "userPublicKey": user.to_string(),
    });

    let swap_res = client
        .post(swap_api)
        .json(&swap_json)
        .send()
        .await
        .map_err(|x| {
            tracing::error!("Error getting swap response from jupiter : {x:?}");
            JupiterError::NetworkError
        })?;
    let swap_res_status = swap_res.status();
    if !swap_res_status.is_success() {
        error!("Swap request was not successful");
        return Err(JupiterError::ErrorBuildingSwap);
    }
    let res_value = swap_res.json::<Value>().await.map_err(|x| {
        error!("Error parsing returned swap instructions: {x:?}");
        JupiterError::ErrorBuildingSwap
    })?;

    let setup = res_value["setupInstructions"]
        .as_array()
        .ok_or(JupiterError::NoInstructionRecieved)
        .inspect_err(|_x| {
            error!(
                "could not find swap instructions in the jup response: {:?}",
                res_value
            );
        })?;
    let cleanup = res_value.get("cleanupInstruction");
    let swap = res_value
        .get("swapInstruction")
        .ok_or(JupiterError::NoInstructionRecieved)
        .inspect_err(|_x| {
            error!(
                "could not find swap instructions in the jup response: {:?}",
                res_value
            );
        })?;

    let mut lookups_array = vec![];
    let lookups = res_value
        .get("addressLookupTableAddresses")
        .ok_or(JupiterError::NoInstructionRecieved)
        .inspect_err(|x| {
            error!(
                "could not find lookup addresses in the jup response: {:?}",
                res_value
            );
        })?
        .as_array()
        .ok_or(JupiterError::NoInstructionRecieved)
        .inspect_err(|x| {
            error!(
                "could not find lookup addresses in the jup response: {:?}",
                res_value
            );
        })?;
    for lookup in lookups {
        let look_up = lookup
            .as_str()
            .ok_or(JupiterError::LookupAddressMissing)
            .inspect_err(|x| {
                error!(
                    "could not find lookup address in the jup response: {:?}",
                    res_value
                );
            })?;
        let lookup_pub = Pubkey::from_str(look_up).map_err(|x| {
            error!(
                "could not parse address in the jup response: {:?}",
                res_value
            );
            JupiterError::ErrorParsingPubkey
        })?;

        let lookup = sol_client
            .get_account_data(&lookup_pub)
            .await
            .map_err(|x| {
                // error!("Error getting lookup account data");
                // error!("{:?}", x);
                error!("error getting lookup data: {:?}", x);
                JupiterError::ErrorGettingData
            })?;

        let deserialized = AddressLookupTable::deserialize(&lookup)
            .map_err(|x| {
                error!("Error deserializing lookup data : {x:?}");
                JupiterError::ErrorDeserializingData
            })?
            .addresses
            .into_owned();

        let parsed: AddressLookupTableAccount = AddressLookupTableAccount {
            key: lookup_pub,
            addresses: deserialized,
        };
        lookups_array.push(parsed);
    }

    {
        for setup in setup {
            instructions.push(parse_instruction_from_value(setup).inspect_err(|x| {
                error!("Failed to parse a setup instruction: {x:?}");
            })?);
        }
    }
    let swap_instruction = parse_instruction_from_value(swap).inspect_err(|x| {
        // error!("Failed to parse swap instruction");
        error!("Failed to parse a swap instruction: {x:?}");
    })?;
    instructions.push(swap_instruction);
    if let Some(cleaup_val) = cleanup {
        if !cleaup_val.is_null() {
            let cleanup = parse_instruction_from_value(cleaup_val).inspect_err(|x| {
                error!("Failed to parse a cleanip instruction: {x:?}");
            })?;
            instructions.push(cleanup);
        } else {
            info!("cleanup is null and not required!")
        }
    };
    match serde_json::from_value::<PartiallyTypedQuote>(quote.clone()) {
        Ok(typed_quote) => {
            println!("got quote");
            let wsol = "So11111111111111111111111111111111111111112";
            let config = MutradeConfig::get_mutrade_config();
            let fee_bps = config.swap_fee_bps.unwrap_or(100);
            if typed_quote.input_mint == wsol {
                let fee_ix = calculate_fee(
                    (typed_quote.in_amount.parse::<u64>().unwrap() * fee_bps) / 10000,
                    user,
                )
                .unwrap();
                instructions.extend(fee_ix);
            }
            // safe to do cause it wont be both as jup disallows same mint swapping
            if typed_quote.output_mint == wsol {
                let fee_ix = calculate_fee(
                    (typed_quote.out_amount.parse::<u64>().unwrap() * fee_bps) / 10000,
                    user,
                )
                .unwrap();
                instructions.extend(fee_ix);
            }
            // todo handle for non sol pair -
            // charge fee still in sol(val of swap) or create token account for the fee collectors (will be expensive for users)
        }
        Err(x) => eprintln!("{:?}", x),
    }

    Ok((instructions, lookups_array))
}
fn parse_instruction_from_value(instruction_str: &Value) -> Result<Instruction, JupiterError> {
    let program_id = instruction_str
        .get("programId")
        .ok_or(JupiterError::GotNone)?
        .as_str()
        .ok_or(JupiterError::GotNone)?;
    let program_id = Pubkey::from_str(program_id).map_err(|x| JupiterError::GotNone)?;
    let accounts_arr = instruction_str["accounts"]
        .as_array()
        .ok_or(JupiterError::GotNone)?
        .clone();
    let data = instruction_str["data"]
        .as_str()
        .ok_or(JupiterError::GotNone)?;
    let decoded_bytes = BASE64_STANDARD
        .decode(data)
        .map_err(|x| JupiterError::GotNone)?;
    // let decoded_bytes = base64::engine::general_purpose::STANDARD
    // .decode(data)
    // .map_err(|x| JupiterError::GotNone)?;
    let instruction = Instruction {
        program_id,
        accounts: {
            let mut accounts = Vec::new();
            for acc in accounts_arr {
                let account_pubkey = acc["pubkey"].as_str().ok_or(JupiterError::GotNone)?;
                let is_signer = acc["isSigner"].as_bool().ok_or(JupiterError::GotNone)?;
                let is_writable = acc["isWritable"].as_bool().ok_or(JupiterError::GotNone)?;
                let account_meta = AccountMeta {
                    pubkey: Pubkey::from_str(account_pubkey)
                        .map_err(|_x| JupiterError::ErrorBuildingKey)?,
                    is_signer,
                    is_writable,
                };
                accounts.push(account_meta);
            }
            accounts
        },
        data: decoded_bytes,
    };

    Ok(instruction)
}
pub fn calculate_fee(total_fee: u64, payer: &Pubkey) -> Result<Vec<Instruction>, ()> {
    let config = MutradeConfig::get_mutrade_config();
    let mut instructions = vec![];
    let mut total_distributed = 0;
    for (i, share) in config.fee_shares.iter().enumerate() {
        let percentage_basis_points = share.fee_bps;
        let address = share.address.parse().map_err(|x| ())?;
        let mut share_fee = total_fee * percentage_basis_points / 10_000;
        // If it's the last share, assign any remaining lamports
        if i == config.fee_shares.len() - 1 {
            share_fee = total_fee - total_distributed;
        }
        total_distributed += share_fee;
        instructions.push(system_instruction::transfer(&payer, &address, share_fee));
    }
    Ok(instructions)
}
