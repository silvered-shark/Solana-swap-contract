pub mod cpmm {
    use serde_with::{DisplayFromStr, serde_as};
    #[derive(Debug, Clone, BorshDeserialize)]
    pub struct Pool {
        pub amm_config: Pubkey,
        pub pool_creator: Pubkey,
        pub token0_vault: Pubkey,
        pub token1_vault: Pubkey,
        pub lp_mint: Pubkey,
        pub token0_mint: Pubkey,
        pub token1_mint: Pubkey,
        pub token0_program: Pubkey,
        pub token1_program: Pubkey,
        pub observation_key: Pubkey,
        pub auth_bump: u8,
        pub status: u8,
        pub lp_mint_decimals: u8,
        pub mint0_decimals: u8,
        pub mint1_decimals: u8,
        pub lp_supply: u64,
        pub protocol_fees_token0: u64,
        pub protocol_fees_token1: u64,
        pub fund_fees_token0: u64,
        pub fund_fees_token1: u64,
        pub open_time: u64,
        pub recent_epoch: u64,
        pub padding: [u64; 31],
    }

    #[serde_as]
    #[derive(Clone, Debug, Default, PartialEq, Eq, BorshDeserialize, Deserialize, Serialize)]
    pub struct RaydiumCpmmSwapEvent {
        #[borsh(skip)]
        pub signature: String,
        #[serde_as(as = "DisplayFromStr")]
        pub pool_id: Pubkey,
        /// pool vault sub trade fees
        pub input_vault_before: u64,
        /// pool vault sub trade fees
        pub output_vault_before: u64,
        /// calculate result without transfer fee
        pub input_amount: u64,
        /// calculate result without transfer fee
        pub output_amount: u64,
        pub input_transfer_fee: u64,
        pub output_transfer_fee: u64,
        pub base_input: bool,
        // pub user: Pubkey,
    }

    #[derive(Clone, Debug, Deserialize, Serialize)]
    pub struct RaydiumParsedCpmmEvent {
        pub user: Pubkey,
        pub input_mint: Pubkey,
        pub output_mint: Pubkey,
        pub input_amount: u64,
        // pub output_amount: u64,
        pub pool_state: Pubkey,
    }
    use borsh::{BorshDeserialize, BorshSerialize};
    use serde::{Deserialize, Serialize};
    use solana_client::nonblocking::rpc_client::RpcClient;
    use solana_program::pubkey;
    use solana_sdk::{
        bs58,
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
    };
    use solana_transaction_status::UiCompiledInstruction;

    pub const POOL_SEED: &[u8] = b"pool";
    pub const POOL_VAULT_SEED: &[u8] = b"pool_vault";
    pub const OBSERVATION_STATE_SEED: &[u8] = b"observation";

    pub const AUTHORITY: Pubkey = pubkey!("GpMZbSM2GgvTKHJirzeGfMFoaZ8UR2X7F4v8vHTvxFbL");
    pub const AMM_CONFIG: Pubkey = pubkey!("D4FPEruKEHrG5TenZ2mpDGEfu1iUvTiqBxvpU8HLBvC2");
    pub const TOKEN_PROGRAM: Pubkey = spl_token::ID;
    pub const WSOL_TOKEN_ACCOUNT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
    pub const SWAP_BASE_IN_DISCRIMINATOR: &[u8] = &[143, 190, 90, 218, 196, 30, 51, 222];
    pub const SWAP_BASE_OUT_DISCRIMINATOR: &[u8] = &[55, 217, 98, 86, 163, 74, 180, 173];
    pub const INITIALIZE_DISCRI: &[u8] = &[175, 175, 109, 31, 13, 152, 155, 237];
    pub const RAYDIUM_CPMM: &str = "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C";
    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct RaydiumBuyBaseIn {
        pub amount_in: u64,
        pub minimum_amount_out: u64,
    }

    #[derive(BorshSerialize, BorshDeserialize)]
    pub struct RaydiumCpmmInit {
        init_amount_0: u64,
        init_amount_1: u64,
        open_time: u64,
    }

    pub async fn parse_raydium_cpmm_ix(data: &str, accounts: Vec<String>) {
        let decoded = bs58::decode(data).into_vec().unwrap();
        let discriminator = &decoded[..8];
        match &discriminator {
            &[143, 190, 90, 218, 196, 30, 51, 222] => {
                // swap base in
            }
            &[55, 217, 98, 86, 163, 74, 180, 173] => {
                // swap base out
            }

            _ => {}
        }
    }

    impl RaydiumBuyBaseIn {
        pub const DISCRIMINATOR: [u8; 8] = [143, 190, 90, 218, 196, 30, 51, 222];

        pub fn data(&self) -> Vec<u8> {
            let mut data = Vec::with_capacity(256);
            data.extend_from_slice(&Self::DISCRIMINATOR);
            self.serialize(&mut data).unwrap();
            data
        }
    }
    fn get_radium_clmm_vault_pda(pool_state: &Pubkey, mint: &Pubkey) -> Option<Pubkey> {
        let seeds: &[&[u8]; 3] = &[POOL_VAULT_SEED, pool_state.as_ref(), mint.as_ref()];
        let program_id: &Pubkey = &RAYDIUM_CPMM.parse().ok()?;
        let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
        pda.map(|pubkey| pubkey.0)
    }
    pub fn get_observation_state_pda(pool_state: &Pubkey) -> Option<Pubkey> {
        let seeds: &[&[u8]; 2] = &[OBSERVATION_STATE_SEED, pool_state.as_ref()];
        let program_id: &Pubkey = &RAYDIUM_CPMM.parse().ok()?;
        let pda: Option<(Pubkey, u8)> = Pubkey::try_find_program_address(seeds, program_id);
        pda.map(|pubkey| pubkey.0)
    }

    pub async fn get_buy_token_amount(
        token0_balance: u128,
        token1_balance: u128,
        pool: &Pool,
        sol_amount: u64,
    ) -> Result<u64, ()> {
        let is_token0_input = if pool.token0_mint == WSOL_TOKEN_ACCOUNT {
            true
        } else {
            false
        };
        let (reserve_in, reserve_out) = if is_token0_input {
            (token0_balance, token1_balance)
        } else {
            (token1_balance, token0_balance)
        };

        if reserve_in == 0 || reserve_out == 0 {
            // return Err(anyhow!("池子储备金为零，无法进行交换"));
            return Err(());
        }

        // 使用 u128 防止溢出
        let amount_in_128 = sol_amount as u128;
        let reserve_in_128 = reserve_in as u128;
        let reserve_out_128 = reserve_out as u128;

        // 恒定乘积公式: amount_out = (amount_in * reserve_out) / (reserve_in + amount_in)
        let numerator = amount_in_128 * reserve_out_128;
        let denominator = reserve_in_128 + amount_in_128;

        if denominator == 0 {
            // return Err(anyhow!("分母为零，计算错误"));
            return Err(());
        }

        let amount_out = numerator / denominator;

        // 检查是否超出储备金
        if amount_out >= reserve_out_128 {
            // return Err(anyhow!("输出数量超过池子储备金"));
            return Err(());
        }
        Ok(amount_out as u64)
    }

    pub async fn get_sell_sol_amount(
        token0_balance: u128,
        token1_balance: u128,
        pool: &Pool,
        token_amount: u64,
    ) -> Result<u64, ()> {
        let is_token0_sol = if pool.token0_mint == WSOL_TOKEN_ACCOUNT {
            true
        } else {
            false
        };
        let (reserve_in, reserve_out) = if is_token0_sol {
            (token1_balance, token0_balance)
        } else {
            (token0_balance, token1_balance)
        };

        if reserve_in == 0 || reserve_out == 0 {
            return Err(());
        }

        // 使用 u128 防止溢出
        let amount_in_128 = token_amount as u128;
        let reserve_in_128 = reserve_in as u128;
        let reserve_out_128 = reserve_out as u128;

        // 恒定乘积公式: amount_out = (amount_in * reserve_out) / (reserve_in + amount_in)
        let numerator = amount_in_128 * reserve_out_128;
        let denominator = reserve_in_128 + amount_in_128;

        if denominator == 0 {
            return Err(());
        }

        let amount_out = numerator / denominator;

        // 检查是否超出储备金
        if amount_out >= reserve_out_128 {
            return Err(());
        }

        Ok(amount_out as u64)
    }
    pub async fn get_pool_token_balances(
        rpc: &RpcClient,
        pool_state: &Pubkey,
        token0_mint: &Pubkey,
        token1_mint: &Pubkey,
    ) -> Result<(u64, u64), ()> {
        let token0_vault = get_radium_clmm_vault_pda(pool_state, token0_mint).unwrap();
        let token0_balance = rpc.get_token_account_balance(&token0_vault).await.unwrap();
        let token1_vault = get_radium_clmm_vault_pda(pool_state, token1_mint).unwrap();
        let token1_balance = rpc.get_token_account_balance(&token1_vault).await.unwrap();
        let token0_amount = token0_balance.amount.parse::<u64>().map_err(|e| ())?;
        let token1_amount = token1_balance.amount.parse::<u64>().map_err(|e| ())?;

        Ok((token0_amount, token1_amount))
    }

    impl Pool {
        pub fn from_bytes(data: &[u8]) -> Result<Self, std::io::Error> {
            let pool = Pool::try_from_slice(&data[8..])?;
            Ok(pool)
        }
        pub async fn fetch(rpc: &RpcClient, pool_address: &Pubkey) -> Result<Self, ()> {
            let account = rpc.get_account(pool_address).await.map_err(|x| ())?;

            if account.owner != RAYDIUM_CPMM.parse().unwrap() {
                return Err(());
            }
            Self::from_bytes(&account.data).map_err(|x| ())
        }
    }
}
