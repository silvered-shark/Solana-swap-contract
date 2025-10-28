use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct SolanaIx {
    #[serde(alias = "type")]
    pub ix_type: String,
}
// #[derive(Debug, Deserialize, Serialize)]
// #[serde(tag = "type", content = "info", rename_all = "camelCase")]
// pub enum SolanaInstruction {
//     GetAccountDataSize {
//         mint: String,
//         #[serde(rename = "extensionTypes")]
//         extension_types: Vec<String>,
//     },
//     CreateAccount {
//         source: String,
//         #[serde(alias = "newAccount")]
//         new_account: String,
//         lamports: u64,
//         space: u64,
//         owner: String,
//     },
//     InitializeImmutableOwner {
//         account: String,
//     },
//     InitializeAccount3 {
//         account: String,
//         mint: String,
//         owner: String,
//     },
//     Transfer {
//         source: String,
//         destination: String,
//         // #[serde(alias = "amount")]
//         // lamports: u64,
//         // authority: String,
//     },
//     TransferChecked {
//         source: String,
//         mint: String,
//         destination: String,
//         #[serde(alias = "tokenAmount")]
//         token_amount: TokenAmount,
//         authority: String,
//     },
//     Create(CreateInfo),               // <--i
//     Allocate(AllocateInfo),           // <-- New variant here
//     InitializeMint2(InitializeMint2), // <--- new variant here
//     CloseAccount(CloseAccountInfo),
//     Assign(AssignInfo),
//     CreateIdempotent(CreateIdempotentInfo),
//     BurnChecked(BurnCheckedInfo),
//     SyncNative {
//         account: String,
//     },
//     MintTo(MintToInfo),
//     Burn(BurnInfo),
// }
// #[derive(Debug, Deserialize, Serialize)]
// #[serde(rename_all = "camelCase")]
// pub struct BurnInfo {
//     pub account: String,
//     pub mint: String,
//     pub amount: String,
//     pub authority: String,
// }
// #[derive(Debug, Deserialize, Serialize)]
// #[serde(rename_all = "camelCase")]
// pub struct MintToInfo {
//     pub mint: String,
//     pub account: String,
//     pub amount: String,
//     pub mint_authority: String,
// }
// #[derive(Debug, Deserialize, Serialize)]
// #[serde(rename_all = "camelCase")]
// pub struct BurnCheckedInfo {
//     pub account: String,
//     pub mint: String,
//     pub token_amount: TokenAmount,
//     pub authority: String,
// }
// #[derive(Debug, Deserialize, Serialize)]
// #[serde(rename_all = "camelCase")]
// pub struct CreateIdempotentInfo {
//     pub source: String,
//     pub account: String,
//     pub wallet: String,
//     pub mint: String,
//     pub system_program: String,
//     pub token_program: String,
// }
// #[derive(Debug, Deserialize, Serialize)]
// #[serde(rename_all = "camelCase")]
// pub struct AssignInfo {
//     pub account: String,
//     pub owner: String,
// }
// #[derive(Debug, Deserialize, Serialize)]
// #[serde(rename_all = "camelCase")]
// pub struct CloseAccountInfo {
//     pub account: String,
//     pub destination: String,
//     pub owner: String,
// }
// #[derive(Debug, Deserialize, Serialize)]
// #[serde(rename_all = "camelCase")]
// pub struct CreateInfo {
//     pub source: String,
//     pub account: String,
//     pub wallet: String,
//     pub mint: String,
//     pub system_program: String,
//     pub token_program: String,
// }
// #[derive(Debug, Deserialize, Serialize)]
// #[serde(rename_all = "camelCase")]
// pub struct TokenAmount {
//     pub ui_amount: f64,
//     pub decimals: u8,
//     pub amount: String,
//     pub ui_amount_string: String,
// }

// #[derive(Debug, Deserialize, Serialize)]
// #[serde(rename_all = "camelCase")]
// pub struct AllocateInfo {
//     pub account: String,
//     pub space: u64,
// }

// #[derive(Debug, Deserialize, Serialize)]
// #[serde(rename_all = "camelCase")]
// pub struct InitializeMint2 {
//     pub mint: String,
//     pub decimals: u8,
//     pub mint_authority: String,
// }
