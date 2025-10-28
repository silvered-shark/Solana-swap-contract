use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use std::u8;

use base64::Engine;
use base64::alphabet::STANDARD;
use base64::prelude::BASE64_STANDARD;
use borsh::BorshDeserialize;
use crossbeam_queue::ArrayQueue;
use db::redis::Commands;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use futures::pin_mut;
use serde::Deserialize;
use serde::Serialize;
use sol_platforms::orca;
use sol_platforms::orca::ORCA_ADDRESS;
use sol_platforms::pump_fun;
use sol_platforms::pump_fun::PUMPFUN_ADDRESS;
use sol_platforms::pump_fun::PumpFun;
use sol_platforms::pump_fun::PumpFunEvent;
use sol_platforms::raydium_clmm::RAYDIUM_CLMM_ADDRESS;
use sol_platforms::raydium_clmm::RaydiumClmmSwapEvent;
use solana_pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_transaction_status::EncodedTransactionWithStatusMeta;
use solana_transaction_status::UiParsedInstruction;
use yellowstone_grpc_proto::geyser::SubscribeUpdate;
use yellowstone_grpc_proto::geyser::SubscribeUpdateTransaction;
use yellowstone_grpc_proto::{geyser::subscribe_update::UpdateOneof, tonic::Status};

use crate::sol::sol_events::sol_platforms::orca::Traded;
use crate::sol::sol_events::sol_platforms::pump_fun::PumpFunCreateEvent;
use crate::sol::sol_events::sol_platforms::pump_fun::PumpFunTradeEvent;
use crate::sol::sol_events::sol_platforms::raydium_cpmm::cpmm::RAYDIUM_CPMM;
use crate::sol::sol_events::sol_platforms::raydium_cpmm::cpmm::RaydiumCpmmSwapEvent;
use crate::sol::sol_events::sol_system_ix::SolanaIx;
pub mod sol_platforms;
pub mod sol_system_ix;
#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum MutEvents {
    TradeEvent(TradeEvent),
    CreateEvent(CreateEvent),
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum TradeEvent {
    PumpFunTrade(PumpFunTradeEvent),
    RaydiumClmmTrade(RaydiumClmmSwapEvent),
    RaydiumCpmmTrade(RaydiumCpmmSwapEvent),
    OrcaTrade(Traded),
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum CreateEvent {
    PumpFunCreate(PumpFunCreateEvent),
}

#[derive(Debug, Clone)]
pub struct ProgramData {
    pub program_id: String,
    pub data: String,
}
pub async fn stream_events(
    result_queue: Arc<ArrayQueue<Vec<MutEvents>>>,
    geyser: impl Stream<Item = Result<SubscribeUpdate, Status>>,
) {
    pin_mut!(geyser);
    let queue = Arc::new(ArrayQueue::new(200));

    for i in 0..1 {
        thread::spawn({
            let queue = queue.clone(); // or pass the original Arc here
            let result_queue = result_queue.clone();
            move || {
                let res = listener(queue, result_queue);
            }
        });
    }
    while let Some(Ok(item)) = geyser.next().await {
        match item.update_oneof {
            Some(update) => match update {
                UpdateOneof::Account(subscribe_update_account) => todo!(),
                UpdateOneof::Slot(subscribe_update_slot) => todo!(),
                UpdateOneof::Transaction(subscribe_update_transaction) => {
                    let now = Instant::now();
                    let res = queue.push(subscribe_update_transaction);
                    println!("{:?}", res);
                }
                UpdateOneof::TransactionStatus(subscribe_update_transaction_status) => todo!(),
                UpdateOneof::Block(subscribe_update_block) => todo!(),
                UpdateOneof::Ping(subscribe_update_ping) => continue,
                UpdateOneof::Pong(subscribe_update_pong) => todo!(),
                UpdateOneof::BlockMeta(subscribe_update_block_meta) => todo!(),
                UpdateOneof::Entry(subscribe_update_entry) => todo!(),
            },
            None => todo!(),
        }
    }
}

pub fn listener(
    queue: Arc<ArrayQueue<SubscribeUpdateTransaction>>,
    result_queue: Arc<ArrayQueue<Vec<MutEvents>>>,
) -> Result<(), ()> {
    // let mut client = db::kv_store::get_kv_client().map_err(|_| ())?;
    let id = std::thread::current().id();
    let mut stack: Vec<(String, u32)> = Vec::new();
    let mut program_data_list: Vec<ProgramData> = Vec::new();
    let mut carrier = Vec::new();
    loop {
        let pop = queue.pop();
        if pop.is_none() {
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        let res = transaction_update_matcher(
            pop.ok_or(())?,
            &mut carrier,
            &mut stack,
            &mut program_data_list,
        )
        .map_err(|_| ())?;
        stack.clear();
        program_data_list.clear();
        if !carrier.is_empty() {
            let as_bytes = serde_json::to_vec(&carrier).map_err(|_| ())?;
            // println!("Publishing to redis");
            // let i: () = client.publish("events", as_bytes).map_err(|_| ())?;
            result_queue.push(carrier.clone());
            carrier.clear();
        } else {
            println!("got empty");
        }
        let pressure = queue.len();
    }
}

pub fn transaction_update_matcher(
    tx: SubscribeUpdateTransaction,
    carrier: &mut Vec<MutEvents>,
    stack: &mut Vec<(String, u32)>,
    program_data_list: &mut Vec<ProgramData>,
) -> Result<(), ()> {
    let tx_info = match tx.transaction {
        Some(x) => x,
        None => todo!(),
    };
    let signature_unp = Signature::try_from(tx_info.signature.as_slice()).map_err(|_| ())?;
    let tx_with_meta = yellowstone_grpc_proto::convert_from::create_tx_with_meta(tx_info)
        .map_err(|_| ())?
        .encode(
            solana_transaction_status::UiTransactionEncoding::Base64,
            Some(u8::MAX),
            false,
        )
        .map_err(|_| ())?;
    let logs = tx_with_meta.meta.ok_or(())?.log_messages.ok_or(())?;
    let events = parse_logs(
        logs,
        carrier,
        signature_unp.to_string(),
        stack,
        program_data_list,
    );

    // parse_events(tx_with_meta, signature_unp.to_string());

    Ok(())
}

fn parse_logs(
    logs: Vec<String>,
    carrier: &mut Vec<MutEvents>,
    signature: String,
    stack: &mut Vec<(String, u32)>,
    program_data_list: &mut Vec<ProgramData>,
) -> Result<(), ()> {
    let parse = parse_solana_logs(logs, stack, program_data_list);
    for ProgramData { program_id, data } in parse.iter() {
        let decoded = match BASE64_STANDARD.decode(data) {
            Ok(x) => x,
            Err(x) => {
                continue;
            }
        };
        match &program_id.as_str() {
            &PUMPFUN_ADDRESS => match &decoded[..8] {
                [27, 114, 169, 77, 222, 235, 99, 118] => {
                    let mut res = PumpFunCreateEvent::try_from_slice(&decoded[8..])
                        .map_err(|x| ())
                        .ok()
                        .ok_or(())?;
                    res.signature = signature.clone();
                    carrier.push(MutEvents::CreateEvent(CreateEvent::PumpFunCreate(res)))
                }
                [189, 219, 127, 211, 78, 230, 97, 238] => {
                    let mut res = match PumpFunTradeEvent::try_from_slice(&decoded[8..]) {
                        Ok(x) => x,
                        Err(e) => {
                            return Err(());
                        }
                    };
                    res.signature = signature.clone();
                    carrier.push(MutEvents::TradeEvent(TradeEvent::PumpFunTrade(res)))
                }
                _ => (),
            },
            &RAYDIUM_CLMM_ADDRESS => match &decoded[..8] {
                [64, 198, 205, 232, 38, 8, 113, 226] => {
                    let mut res = match RaydiumClmmSwapEvent::try_from_slice(&decoded[8..]) {
                        Ok(x) => x,
                        Err(e) => {
                            return Err(());
                        }
                    };
                    res.signature = signature.clone();
                    carrier.push(MutEvents::TradeEvent(TradeEvent::RaydiumClmmTrade(res)))
                }
                _ => (),
            },
            &RAYDIUM_CPMM => match &decoded[..8] {
                [64, 198, 205, 232, 38, 8, 113, 226] => {
                    let mut res = match RaydiumCpmmSwapEvent::try_from_slice(&decoded[8..]) {
                        Ok(x) => x,
                        Err(e) => {
                            return Err(());
                        }
                    };
                    res.signature = signature.clone();
                    carrier.push(MutEvents::TradeEvent(TradeEvent::RaydiumCpmmTrade(res)))
                }
                _ => (),
            },

            &ORCA_ADDRESS => match &decoded[..8] {
                [225, 202, 73, 175, 147, 43, 160, 150] => {
                    let mut res = match Traded::try_from_slice(&decoded[8..]) {
                        Ok(x) => x,
                        Err(e) => {
                            return Err(());
                        }
                    };
                    res.signature = signature.clone();
                    carrier.push(MutEvents::TradeEvent(TradeEvent::OrcaTrade(res)))
                }
                _ => (),
            },

            _ => {}
        }
    }

    println!("Returning with a len of {}", carrier.len());

    // panic!();
    Ok(())
}

pub fn event_handler(decoded: Vec<u8>, signature: String) -> Result<MutEvents, ()> {
    match &decoded[..8] {
        _ => return Err(()),
    }
}
pub fn parse_instructions(
    tx_encoded: EncodedTransactionWithStatusMeta,
    signature: String,
) -> Result<Vec<MutEvents>, ()> {
    let tx = tx_encoded.meta.ok_or(())?;
    let ixs = tx.inner_instructions.ok_or(())?;

    for inner_ixns in ixs {
        let main_index = inner_ixns.index;
        for (i, ix) in inner_ixns.instructions.into_iter().enumerate() {
            match ix {
                solana_transaction_status::UiInstruction::Parsed(ui_parsed_instruction) => {
                    match ui_parsed_instruction {
                        UiParsedInstruction::Parsed(parsed_instruction) => {
                            todo!()
                        }
                        UiParsedInstruction::PartiallyDecoded(x) => {
                            let program = x.program_id;

                            if program
                                .eq_ignore_ascii_case("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr")
                            {
                                panic!("got it");
                            }
                        }
                    }
                }
                solana_transaction_status::UiInstruction::Compiled(ui_compiled_instruction) => {
                    let versioned_tx = tx_encoded.transaction.decode().unwrap();
                    let mut accounts = versioned_tx.message.static_account_keys().to_vec();
                    let loaded_addresses = tx.loaded_addresses.clone().unwrap();
                    for loaded_address in loaded_addresses.writable {
                        accounts.push(Pubkey::from_str(&loaded_address).unwrap());
                    }
                    for loaded_address in loaded_addresses.readonly {
                        accounts.push(Pubkey::from_str(&loaded_address).unwrap());
                    }
                    let program_id_index = ui_compiled_instruction.program_id_index;
                    let program_id = accounts[program_id_index as usize];

                    match program_id.to_string().as_str() {
                        RAYDIUM_CLMM_ADDRESS => {}
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(vec![])
}

fn parse_solana_logs<'a>(
    logs: Vec<String>,
    stack: &'a mut Vec<(String, u32)>,
    program_data_list: &'a mut Vec<ProgramData>,
) -> &'a mut Vec<ProgramData> {
    for log_line in logs {
        let trimmed = log_line.trim();
        if let Some((program_id, depth)) = parse_invoke(trimmed) {
            // Push program with its depth to the stack (needed for tracking context)
            stack.push((program_id, depth));
        } else if let Some((program_id, _)) = parse_success(trimmed) {
            // Remove the matching program from the stack
            if let Some(pos) = stack.iter().rposition(|(pid, _)| *pid == program_id) {
                stack.remove(pos);
            }
        } else if trimmed.starts_with("Program data: ") {
            // Only capture Program data logs - attribute to the top program on stack
            let data_content = &trimmed[14..]; // Remove "Program data: " prefix

            if let Some((emitter, _)) = stack.last() {
                program_data_list.push(ProgramData {
                    program_id: emitter.clone(),
                    data: data_content.to_string(),
                });
            } else {
                program_data_list.push(ProgramData {
                    program_id: "UNKNOWN".to_string(),
                    data: data_content.to_string(),
                });
            }
        }
        // Ignore all other log types
    }
    program_data_list
}

// Parse: Program <program_id> invoke [x]
fn parse_invoke(line: &str) -> Option<(String, u32)> {
    let prefix = "Program ";
    let invoke_suffix = " invoke [";
    if line.starts_with(prefix) && line.contains(invoke_suffix) {
        let program_id = line[prefix.len()..].split_whitespace().next()?.to_string();

        // Extract depth level from [x]
        if let Some(start) = line.find('[') {
            if let Some(end) = line.find(']') {
                if let Ok(depth) = line[start + 1..end].parse::<u32>() {
                    return Some((program_id, depth));
                }
            }
        }
        return Some((program_id, 1));
    }
    None
}

// Parse: Program <program_id> success
fn parse_success(line: &str) -> Option<(String, u32)> {
    let prefix = "Program ";
    let suffix = " success";
    if line.starts_with(prefix) && line.ends_with(suffix) {
        let program_id = &line[prefix.len()..line.len() - suffix.len()];
        return Some((program_id.trim().to_string(), 0)); // depth not important for success
    }
    None
}
