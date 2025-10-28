use base64::{Engine, prelude::BASE64_STANDARD};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_keypair::Keypair;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
// use solana_pubkey::Pubkey;
use serde_with::{DisplayFromStr, serde_as};
use solana_pubkey::pubkey;
use solana_signer::Signer;
use spl_associated_token_account::get_associated_token_address;

use crate::sol::sol_events::MutEvents;

pub const GLOBAL_SEED: &[u8] = b"global";

pub const FEE_BASIS_POINTS: u64 = 95;
pub const CREATOR_FEE: u64 = 5;
pub const GLOBAL_VOLUME_ACCUMULATOR: Pubkey =
    pubkey!("Hq2wp8uJ9jCPsYgNHex8RtqdvMPfVGoYwjvF1ATiwn2Y");

/// Seed for the mint authority PDA
pub const MINT_AUTHORITY_SEED: &[u8] = b"mint-authority";

/// Seed for bonding curve PDAs
pub const BONDING_CURVE_SEED: &[u8] = b"bonding-curve";

/// Seed for metadata PDAs
pub const METADATA_SEED: &[u8] = b"metadata";

/// Seed for creator vault PDA
pub const CREATOR_VAULT_SEED: &[u8] = b"creator-vault";

pub const PUMPFUN: Pubkey = pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");

/// Public key for the MPL Token Metadata program
pub const MPL_TOKEN_METADATA: Pubkey = pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

/// Authority for program events
pub const EVENT_AUTHORITY: Pubkey = pubkey!("Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1");

/// System Program ID
pub const SYSTEM_PROGRAM: Pubkey = pubkey!("11111111111111111111111111111111");

/// Token Program ID
pub const TOKEN_PROGRAM: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// Associated Token Program ID
pub const ASSOCIATED_TOKEN_PROGRAM: Pubkey =
    pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

/// Rent Sysvar ID
pub const RENT: Pubkey = pubkey!("SysvarRent111111111111111111111111111111111");
pub const PUMP_FUN_BUY_DISCRIMINATOR: &[u8; 8] = &[102, 6, 61, 18, 1, 218, 235, 234];
pub const PUMP_FUN_SELL_DISCRIMINATOR: &[u8; 8] = &[51, 230, 133, 164, 1, 127, 131, 173];
pub const PUMPFUN_ADDRESS: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
pub struct PumpFun;
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct BondingCurveAccount {
    /// Unique identifier for the bonding curve
    pub discriminator: u64,
    /// Virtual token reserves used for price calculations
    pub virtual_token_reserves: u64,
    /// Virtual SOL reserves used for price calculations
    pub virtual_sol_reserves: u64,
    /// Actual token reserves available for trading
    pub real_token_reserves: u64,
    /// Actual SOL reserves available for trading
    pub real_sol_reserves: u64,
    /// Total supply of tokens
    pub token_total_supply: u64,
    /// Whether the bonding curve is complete/finalized
    pub complete: bool,
    /// Token creator's address
    pub creator: Pubkey,
}

impl BondingCurveAccount {}
impl BondingCurveAccount {
    pub fn calculate_market_cap_fixed_point(curve: &BondingCurveAccount) {
        let circulating_supply = curve
            .token_total_supply
            .checked_sub(curve.real_token_reserves)
            .expect("real_token_reserves should be <= total_supply");

        let total_sol = (curve.virtual_sol_reserves as u128)
            .checked_add(curve.real_sol_reserves as u128)
            .expect("SOL reserves addition overflow");

        let total_token = curve
            .virtual_token_reserves
            .checked_add(curve.real_token_reserves)
            .expect("Token reserves addition overflow");

        let scale = 1_000_000_000u128; // fixed-point scale for 9 decimals

        let price_per_token_scaled = total_sol
            .checked_mul(scale)
            .expect("Multiplication overflow")
            .checked_div(total_token as u128)
            .expect("Division by zero");

        let market_cap_scaled = circulating_supply
            .checked_mul(price_per_token_scaled as u64)
            .expect("Market cap multiplication overflow");

        let market_cap = market_cap_scaled
            .checked_div(scale as u64)
            .expect("Division by zero");

        println!("Circulating Supply: {}", circulating_supply);
        println!("Total SOL (lamports): {}", total_sol);
        println!("Total Token: {}", total_token);
        println!(
            "Price per token (scaled by 1e9): {}",
            price_per_token_scaled
        );
        println!("Market cap (scaled by 1e9): {}", market_cap_scaled);
        println!("Market cap (lamports): {}", market_cap);
    }

    /// Creates a new bonding curve instance
    ///
    /// # Arguments
    /// * `discriminator` - Unique identifier for the curve
    /// * `virtual_token_reserves` - Virtual token reserves for price calculations
    /// * `virtual_sol_reserves` - Virtual SOL reserves for price calculations
    /// * `real_token_reserves` - Actual token reserves available
    /// * `real_sol_reserves` - Actual SOL reserves available
    /// * `token_total_supply` - Total supply of tokens
    /// * `complete` - Whether the curve is complete
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        discriminator: u64,
        virtual_token_reserves: u64,
        virtual_sol_reserves: u64,
        real_token_reserves: u64,
        real_sol_reserves: u64,
        token_total_supply: u64,
        complete: bool,
        creator: Pubkey,
    ) -> Self {
        Self {
            discriminator,
            virtual_token_reserves,
            virtual_sol_reserves,
            real_token_reserves,
            real_sol_reserves,
            token_total_supply,
            complete,
            creator,
        }
    }

    /// Calculates the amount of tokens received for a given SOL amount
    ///
    /// # Arguments
    /// * `amount` - Amount of SOL to spend
    ///
    /// # Returns
    /// * `Ok(u64)` - Amount of tokens that would be received
    /// * `Err(&str)` - Error message if curve is complete
    pub fn get_buy_price(&self, amount: u64) -> Result<u64, &'static str> {
        if self.complete {
            return Err("Curve is complete");
        }

        if amount == 0 {
            return Ok(0);
        }

        // Calculate the product of virtual reserves using u128 to avoid overflow
        let n: u128 = (self.virtual_sol_reserves as u128) * (self.virtual_token_reserves as u128);

        // Calculate the new virtual sol reserves after the purchase
        let i: u128 = (self.virtual_sol_reserves as u128) + (amount as u128);

        // Calculate the new virtual token reserves after the purchase
        let r: u128 = n / i + 1;

        // Calculate the amount of tokens to be purchased
        let s: u128 = (self.virtual_token_reserves as u128) - r;

        // Convert back to u64 and return the minimum of calculated tokens and real reserves
        let s_u64 = s as u64;
        Ok(if s_u64 < self.real_token_reserves {
            s_u64
        } else {
            self.real_token_reserves
        })
    }

    /// Calculates the amount of SOL received for selling tokens
    ///
    /// # Arguments
    /// * `amount` - Amount of tokens to sell
    /// * `fee_basis_points` - Fee in basis points (1/100th of a percent)
    ///

    pub fn get_buy_token_amount_from_sol_amount(&self, amount: u64) -> u64 {
        if amount == 0 {
            return 0;
        }

        if self.virtual_token_reserves == 0 {
            return 0;
        }

        let total_fee_basis_points = FEE_BASIS_POINTS
            + if self.creator != Pubkey::default() {
                CREATOR_FEE
            } else {
                0
            };

        // 转为 u128 防止溢出
        let amount_128 = amount as u128;
        let total_fee_basis_points_128 = total_fee_basis_points as u128;
        let input_amount = amount_128
            .checked_mul(10_000)
            .unwrap()
            .checked_div(total_fee_basis_points_128 + 10_000)
            .unwrap();

        let virtual_token_reserves = self.virtual_token_reserves as u128;
        let virtual_sol_reserves = self.virtual_sol_reserves as u128;
        let real_token_reserves = self.real_token_reserves as u128;

        let denominator = virtual_sol_reserves + input_amount;

        let tokens_received = input_amount
            .checked_mul(virtual_token_reserves)
            .unwrap()
            .checked_div(denominator)
            .unwrap();

        tokens_received.min(real_token_reserves) as u64
    }
    /// # Returns
    /// * `Ok(u64)` - Amount of SOL that would be received after fees
    /// * `Err(&str)` - Error message if curve is complete
    pub fn get_sell_price(&self, amount: u64, fee_basis_points: u64) -> Result<u64, &'static str> {
        if self.complete {
            return Err("Curve is complete");
        }

        if amount == 0 {
            return Ok(0);
        }

        // Calculate the proportional amount of virtual sol reserves to be received using u128
        let n: u128 = ((amount as u128) * (self.virtual_sol_reserves as u128))
            / ((self.virtual_token_reserves as u128) + (amount as u128));

        // Calculate the fee amount in the same units
        let a: u128 = (n * (fee_basis_points as u128)) / 10000;

        // Return the net amount after deducting the fee, converting back to u64
        Ok((n - a) as u64)
    }

    /// Calculates the current market cap in SOL
    pub fn get_market_cap_sol(&self) -> u64 {
        if self.virtual_token_reserves == 0 {
            return 0;
        }

        ((self.token_total_supply as u128) * (self.virtual_sol_reserves as u128)
            / (self.virtual_token_reserves as u128)) as u64
    }

    /// Calculates the final market cap in SOL after all tokens are sold
    ///
    /// # Arguments
    /// * `fee_basis_points` - Fee in basis points (1/100th of a percent)
    pub fn get_final_market_cap_sol(&self, fee_basis_points: u64) -> u64 {
        let total_sell_value: u128 =
            self.get_buy_out_price(self.real_token_reserves, fee_basis_points) as u128;
        let total_virtual_value: u128 = (self.virtual_sol_reserves as u128) + total_sell_value;
        let total_virtual_tokens: u128 =
            (self.virtual_token_reserves as u128) - (self.real_token_reserves as u128);

        if total_virtual_tokens == 0 {
            return 0;
        }

        ((self.token_total_supply as u128) * total_virtual_value / total_virtual_tokens) as u64
    }

    /// Calculates the price to buy out all remaining tokens
    ///
    /// # Arguments
    /// * `amount` - Amount of tokens to buy
    /// * `fee_basis_points` - Fee in basis points (1/100th of a percent)
    pub fn get_buy_out_price(&self, amount: u64, fee_basis_points: u64) -> u64 {
        // Get the effective amount of sol tokens
        let sol_tokens: u128 = if amount < self.real_sol_reserves {
            self.real_sol_reserves as u128
        } else {
            amount as u128
        };

        // Calculate total sell value
        let total_sell_value: u128 = (sol_tokens * (self.virtual_sol_reserves as u128))
            / ((self.virtual_token_reserves as u128) - sol_tokens)
            + 1;

        // Calculate fee
        let fee: u128 = (total_sell_value * (fee_basis_points as u128)) / 10000;

        // Return total including fee, converting back to u64
        (total_sell_value + fee) as u64
    }
}
impl PumpFun {
    pub fn get_bonding_curve_pda(mint: &Pubkey) -> Option<Pubkey> {
        let seeds: &[&[u8]; 2] = &[BONDING_CURVE_SEED, mint.as_ref()];
        let program_id: &Pubkey = &PUMPFUN;
        let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
        pda.map(|pubkey| pubkey.0)
    }
    pub fn get_mint_authority_pda() -> Pubkey {
        let seeds: &[&[u8]; 1] = &[MINT_AUTHORITY_SEED];
        let program_id: &Pubkey = &PUMPFUN;
        Pubkey::find_program_address(seeds, program_id).0
    }
    pub fn get_metadata_pda(mint: &Pubkey) -> Pubkey {
        let seeds: &[&[u8]; 3] = &[METADATA_SEED, MPL_TOKEN_METADATA.as_ref(), mint.as_ref()];
        let program_id: &Pubkey = &MPL_TOKEN_METADATA;
        Pubkey::find_program_address(seeds, program_id).0
    }
    pub fn get_creator_vault_pda(creator: &Pubkey) -> Option<Pubkey> {
        let seeds: &[&[u8]; 2] = &[CREATOR_VAULT_SEED, creator.as_ref()];
        let program_id: &Pubkey = &PUMPFUN;
        let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
        pda.map(|pubkey| pubkey.0)
    }
    pub fn get_user_vol_acc(user: &Pubkey) -> Option<Pubkey> {
        let seeds: &[&[u8]; 2] = &[b"user_volume_accumulator", user.as_ref()];
        let program_id: &Pubkey = &PUMPFUN;
        let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
        pda.map(|pubkey| pubkey.0)
    }
    pub fn get_user_volume_accumulator_pda(user: &Pubkey) -> Pubkey {
        let (user_volume_accumulator, _bump) =
            Pubkey::find_program_address(&[b"user_volume_accumulator", user.as_ref()], &PUMPFUN);
        user_volume_accumulator
    }
    pub fn get_global_vol_acc() -> Option<Pubkey> {
        let seeds: &[&[u8]; 1] = &[b"global_volume_accumulator"];
        let program_id: &Pubkey = &PUMPFUN;
        let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
        pda.map(|pubkey| pubkey.0)
    }
    pub async fn get_bonding_curve_account(
        // &self,
        rpc: &RpcClient,
        mint: &Pubkey,
    ) -> Result<BondingCurveAccount, ()> {
        let bonding_curve_pda = Self::get_bonding_curve_pda(mint).ok_or(())?;

        let account = rpc.get_account(&bonding_curve_pda).await.map_err(|x| {
            println!("{:?}", x);
            ()
        })?;

        solana_sdk::borsh1::try_from_slice_unchecked::<BondingCurveAccount>(&account.data).map_err(
            |x| {
                println!("{:?}", x);
                ()
            },
        )
    }
    pub fn get_global_pda() -> Pubkey {
        let seeds: &[&[u8]; 1] = &[GLOBAL_SEED];
        let program_id: &Pubkey = &PUMPFUN;
        Pubkey::find_program_address(seeds, program_id).0
    }
}
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PumpFunBuy {
    pub amount: u64,
    pub max_sol_cost: u64,
}
#[serde_as]
#[derive(borsh::BorshDeserialize, Clone, Debug, Deserialize, Serialize)]
pub struct PumpFunCreateEvent {
    #[borsh(skip)]
    pub signature: String,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    #[serde_as(as = "DisplayFromStr")]
    pub mint: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub bonding_curve: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub user: Pubkey,
    pub creator: Pubkey,
    pub timestamp: i64,
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub token_total_supply: u64,
}
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize, borsh::BorshDeserialize)]
pub struct PumpFunTradeEvent {
    #[borsh(skip)]
    pub signature: String,
    #[serde_as(as = "DisplayFromStr")]
    pub mint: Pubkey,
    pub sol_amount: u64,
    pub token_amount: u64,
    pub is_buy: bool,
    #[serde_as(as = "DisplayFromStr")]
    pub user: Pubkey,
    pub timestamp: i64,
    pub virtual_sol_reserves: u64,
    pub virtual_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub real_token_reserves: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub fee_recipient: Pubkey,
    pub fee_basis_points: u64,
    pub fee: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub creator: Pubkey,
    pub creator_fee_basis_points: u64,
    pub creator_fee: u64,
    pub track_volume: bool,
    pub total_unclaimed_tokens: u64,
    pub total_claimed_tokens: u64,
    pub current_sol_volume: u64,
    pub last_update_timestamp: i64,
    pub ix_name: String,
}
#[derive(Deserialize, Serialize, Debug)]
pub struct PumpFunEvent {
    pub signature: String,
    pub is_buy: bool,
    pub mint: String,
    pub amount: u64,
    pub user: String,
}
impl PumpFunBuy {
    pub const DISCRIMINATOR: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];
    pub fn data(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(256);
        data.extend_from_slice(&Self::DISCRIMINATOR);
        self.serialize(&mut data).unwrap();
        data
    }
}
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct PumpFunSell {
    pub amount: u64,
    pub min_sol_output: u64,
}

impl PumpFunSell {
    pub const DISCRIMINATOR: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];
    pub fn data(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(256);
        data.extend_from_slice(&Self::DISCRIMINATOR);
        self.serialize(&mut data).unwrap();
        data
    }
}

// create
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct CreatePumpFun {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub creator: Pubkey,
}

impl CreatePumpFun {
    /// Instruction discriminator used to identify this instruction
    pub const DISCRIMINATOR: [u8; 8] = [24, 30, 200, 40, 5, 28, 7, 119];
    /// Byte vector containing the serialized instruction data
    pub fn data(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(256);
        data.extend_from_slice(&Self::DISCRIMINATOR);
        self.serialize(&mut data).unwrap();
        data
    }
}
pub fn create(payer: &Keypair, mint: &Keypair, args: CreatePumpFun) -> Instruction {
    let bonding_curve: Pubkey = PumpFun::get_bonding_curve_pda(&mint.pubkey()).unwrap();
    Instruction::new_with_bytes(
        PUMPFUN_ADDRESS.parse().unwrap(),
        &args.data(),
        vec![
            AccountMeta::new(mint.pubkey(), true),
            AccountMeta::new(PumpFun::get_mint_authority_pda(), false),
            AccountMeta::new(bonding_curve, false),
            AccountMeta::new(
                get_associated_token_address(&bonding_curve, &mint.pubkey()),
                false,
            ),
            AccountMeta::new_readonly(PumpFun::get_global_pda(), false),
            AccountMeta::new_readonly(MPL_TOKEN_METADATA, false),
            AccountMeta::new(PumpFun::get_metadata_pda(&mint.pubkey()), false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM, false),
            AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM, false),
            AccountMeta::new_readonly(RENT, false),
            AccountMeta::new_readonly(EVENT_AUTHORITY, false),
            AccountMeta::new_readonly(PUMPFUN, false),
        ],
    )
}

// buy

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct Buy {
    pub amount: u64,
    pub max_sol_cost: u64,
}
impl Buy {
    pub const DISCRIMINATOR: [u8; 8] = [102, 6, 61, 18, 1, 218, 235, 234];
    pub fn data(&self) -> Result<Vec<u8>, ()> {
        let mut data = Vec::with_capacity(256);
        data.extend_from_slice(&Self::DISCRIMINATOR);
        self.serialize(&mut data).map_err(|_| ())?;
        Ok(data)
    }
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug)]
pub struct Sell {
    pub amount: u64,
    pub min_sol_output: u64,
}

impl Sell {
    /// Instruction discriminator used to identify this instruction
    pub const DISCRIMINATOR: [u8; 8] = [51, 230, 133, 164, 1, 127, 131, 173];

    /// Serializes the instruction data with the appropriate discriminator
    ///
    /// # Returns
    ///
    /// Byte vector containing the serialized instruction data
    pub fn data(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(256);
        data.extend_from_slice(&Self::DISCRIMINATOR);
        self.serialize(&mut data).unwrap();
        data
    }
}
pub fn sell(
    payer: &Pubkey,
    mint: &Pubkey,
    fee_recipient: &Pubkey,
    creator: &Pubkey,
    args: Sell,
) -> Instruction {
    let bonding_curve: Pubkey = PumpFun::get_bonding_curve_pda(mint).unwrap();
    let creator_vault: Pubkey = PumpFun::get_creator_vault_pda(creator).unwrap();
    Instruction::new_with_bytes(
        PUMPFUN,
        &args.data(),
        vec![
            AccountMeta::new_readonly(PumpFun::get_global_pda(), false),
            AccountMeta::new(*fee_recipient, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(bonding_curve, false),
            AccountMeta::new(get_associated_token_address(&bonding_curve, mint), false),
            AccountMeta::new(get_associated_token_address(&payer, mint), false),
            AccountMeta::new(payer.clone(), true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new(creator_vault, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM, false),
            AccountMeta::new_readonly(EVENT_AUTHORITY, false),
            AccountMeta::new_readonly(PUMPFUN, false),
            AccountMeta::new(
                pubkey!("8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt"),
                false,
            ),
            AccountMeta::new(
                pubkey!("pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ"),
                false,
            ),
            // AccountMeta::new(GLOBAL_VOLUME_ACCUMULATOR, false),
            // AccountMeta::new(
            //     PumpFun::get_user_volume_accumulator_pda(&payer.pubkey()),
            //     false,
            // ),
        ],
    )
}
pub fn buy(
    payer: &Pubkey,
    mint: &Pubkey,
    fee_recipient: &Pubkey,
    creator: &Pubkey,
    args: Buy,
) -> Instruction {
    let bonding_curve: Pubkey = PumpFun::get_bonding_curve_pda(mint).unwrap();
    let creator_vault: Pubkey = PumpFun::get_creator_vault_pda(creator).unwrap();
    Instruction::new_with_bytes(
        PUMPFUN,
        &args.data().unwrap(),
        vec![
            AccountMeta::new_readonly(PumpFun::get_global_pda(), false),
            AccountMeta::new(*fee_recipient, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(bonding_curve, false),
            AccountMeta::new(get_associated_token_address(&bonding_curve, mint), false),
            AccountMeta::new(get_associated_token_address(&payer, mint), false),
            AccountMeta::new(payer.clone(), true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM, false),
            AccountMeta::new(creator_vault, false),
            AccountMeta::new_readonly(EVENT_AUTHORITY, false),
            AccountMeta::new_readonly(PUMPFUN, false),
            AccountMeta::new(GLOBAL_VOLUME_ACCUMULATOR, false),
            AccountMeta::new(PumpFun::get_user_volume_accumulator_pda(&payer), false),
            AccountMeta::new(
                pubkey!("8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt"),
                false,
            ),
            AccountMeta::new(
                pubkey!("pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ"),
                false,
            ),
        ],
    )
}

//
pub struct GlobalAccount {
    /// Unique identifier for the global account
    pub discriminator: u64,
    /// Whether the global account has been initialized
    pub initialized: bool,
    /// Authority that can modify global settings
    pub authority: Pubkey,
    /// Account that receives fees
    pub fee_recipient: Pubkey,
    /// Initial virtual token reserves for price calculations
    pub initial_virtual_token_reserves: u64,
    /// Initial virtual SOL reserves for price calculations
    pub initial_virtual_sol_reserves: u64,
    /// Initial actual token reserves available for trading
    pub initial_real_token_reserves: u64,
    /// Total supply of tokens
    pub token_total_supply: u64,
    /// Fee in basis points (1/100th of a percent)
    pub fee_basis_points: u64,
    /// Authority that can withdraw funds
    pub withdraw_authority: Pubkey,
    /// Flag to enable pool migration
    pub enable_migrate: bool,
    /// Fee for migrating pools
    pub pool_migration_fee: u64,
    /// Fee for creators in base points
    pub creator_fee_basis_points: u64,
    /// Array of public keys for fee recipients
    pub fee_recipients: [Pubkey; 7],
    /// Authority that sets the creator of the token
    pub set_creator_authority: Pubkey,
}

impl GlobalAccount {
    pub fn get_initial_buy_price(&self, amount: u64) -> u64 {
        if amount == 0 {
            return 0;
        }

        let n: u128 = (self.initial_virtual_sol_reserves as u128)
            * (self.initial_virtual_token_reserves as u128);
        let i: u128 = (self.initial_virtual_sol_reserves as u128) + (amount as u128);
        let r: u128 = n / i + 1;
        let s: u128 = (self.initial_virtual_token_reserves as u128) - r;

        if s < (self.initial_real_token_reserves as u128) {
            s as u64
        } else {
            self.initial_real_token_reserves
        }
    }
}

pub struct CreatePumpFunMetadata {
    pub name: String,
    pub symbol: String,
    pub description: String,
    pub twitter: Option<String>,
    pub telegram: Option<String>,
    pub website: Option<String>,
    pub file_bytes: Vec<u8>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenMetadataResponse {
    /// The uploaded token metadata
    /// IPFS URI where the metadata is stored
    pub metadata_uri: String,
}
pub async fn create_token_metadata(
    metadata: CreatePumpFunMetadata,
) -> Result<TokenMetadataResponse, Box<dyn std::error::Error>> {
    let boundary = "------------------------f4d9c2e8b7a5310f";
    let mut body = Vec::new();

    // Helper function to append form data
    fn append_text_field(body: &mut Vec<u8>, boundary: &str, name: &str, value: &str) {
        body.extend_from_slice(b"--");
        body.extend_from_slice(boundary.as_bytes());
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{}\"\r\n\r\n", name).as_bytes(),
        );
        body.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    // Append form fields
    append_text_field(&mut body, boundary, "name", &metadata.name);
    append_text_field(&mut body, boundary, "symbol", &metadata.symbol);
    append_text_field(&mut body, boundary, "description", &metadata.description);
    if let Some(twitter) = metadata.twitter {
        append_text_field(&mut body, boundary, "twitter", &twitter);
    }
    if let Some(telegram) = metadata.telegram {
        append_text_field(&mut body, boundary, "telegram", &telegram);
    }
    if let Some(website) = metadata.website {
        append_text_field(&mut body, boundary, "website", &website);
    }
    append_text_field(&mut body, boundary, "showName", "true");

    // Append file part
    body.extend_from_slice(b"--");
    body.extend_from_slice(boundary.as_bytes());
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"file\"; filename=\"file\"\r\n");
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");

    // Read the file contents
    body.extend_from_slice(&metadata.file_bytes);

    // Close the boundary
    body.extend_from_slice(b"\r\n--");
    body.extend_from_slice(boundary.as_bytes());
    body.extend_from_slice(b"--\r\n");

    let client = reqwest::Client::new();
    let post = client
        .post("https://pump.fun/api/ipfs")
        .header("Content-Length", body.len() as u64)
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(body)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    // Send request and print response

    println!("Upload post: {}", post);

    Ok(serde_json::from_str(&post).unwrap())
}
