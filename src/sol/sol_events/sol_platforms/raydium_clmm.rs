use base64::{Engine, prelude::BASE64_STANDARD};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
// use solana_pubkey::Pubkey;
use serde_with::{DisplayFromStr, serde_as};

use crate::sol::sol_events::MutEvents;
pub const RAYDIUM_CLMM_ADDRESS: &str = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK";
#[serde_as]
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Serialize, Deserialize)]
pub struct RaydiumClmmSwapEvent {
    #[borsh(skip)]
    pub signature: String,
    /// The pool for which token_0 and token_1 were swapped
    #[serde_as(as = "DisplayFromStr")]
    pub pool_state: Pubkey,

    /// The address that initiated the swap call, and that received the callback
    #[serde_as(as = "DisplayFromStr")]
    pub sender: Pubkey,

    /// The payer token account in zero for one swaps, or the recipient token account
    /// in one for zero swaps
    #[serde_as(as = "DisplayFromStr")]
    pub token_account_0: Pubkey,

    /// The payer token account in one for zero swaps, or the recipient token account
    /// in zero for one swaps
    #[serde_as(as = "DisplayFromStr")]
    pub token_account_1: Pubkey,

    /// The real delta amount of the token_0 of the pool or user
    pub amount_0: u64,

    /// The transfer fee charged by the withheld_amount of the token_0
    pub transfer_fee_0: u64,

    /// The real delta of the token_1 of the pool or user
    pub amount_1: u64,

    /// The transfer fee charged by the withheld_amount of the token_1
    pub transfer_fee_1: u64,

    /// If true, amount_0 is negative and amount_1 is positive
    pub zero_for_one: bool,

    /// The sqrt(price) of the pool after the swap, as a Q64.64
    pub sqrt_price_x64: u128,

    /// The liquidity of the pool after the swap
    pub liquidity: u128,

    /// The log base 1.0001 of price of the pool after the swap
    pub tick: i32,
}
