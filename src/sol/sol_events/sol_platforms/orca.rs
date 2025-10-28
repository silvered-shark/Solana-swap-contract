use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use solana_sdk::pubkey::Pubkey;
pub(crate) const ORCA_ADDRESS: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
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
