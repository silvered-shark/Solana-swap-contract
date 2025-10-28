use serde::Deserialize;
use solana_sdk::instruction::AccountMeta;

pub mod jupiter;
#[derive(Deserialize, Debug)]
pub enum SolanaDex {
    Raydium,
    PumpFun,
    PumpSwap,
    Jupiter,
}
