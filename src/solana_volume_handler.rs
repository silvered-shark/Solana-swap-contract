use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader},
    ops::Range,
    str::FromStr,
    thread::{self, Thread},
    time::Duration,
};

use rand::seq::IndexedRandom;
use solana_keypair::Keypair;
use solana_program::system_instruction;
use solana_pubkey::Pubkey;
use solana_sdk::native_token::{lamports_to_sol, sol_str_to_lamports};
use solana_signer::Signer;
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account_idempotent,
};
use spl_token::instruction::close_account;

use crate::sol::{
    SolMut,
    sol_events::sol_platforms::pump_fun::{self, Buy, PumpFun},
};

pub struct VolumeHandle {
    pub fund_wallets: Vec<Keypair>,
    pub mint: solana_sdk::pubkey::Pubkey,
    pub creator: solana_sdk::pubkey::Pubkey,
    pub min_lamports: u64,
    pub max_lamports: u64,
    pub slippage: u64,
    pub sleep_ms_min: i32,
    pub makers: Vec<Keypair>,
    sleep_ms_max: i32,
}

pub async fn handle_pump_volume() -> Result<(), ()> {
    let file = OpenOptions::new()
        .read(true)
        .open("wallets.txt")
        .map_err(|_| ())?;
    let reader = BufReader::new(file);
    let wallets: Vec<String> = reader.lines().collect::<Result<_, _>>().map_err(|_| ())?;

    let mut makers = Vec::with_capacity(wallets.len());
    for wallet in wallets {
        makers.push(Keypair::from_base58_string(&wallet));
    }
    let admins_file = OpenOptions::new()
        .read(true)
        .open("admins.txt")
        .map_err(|_| ())?;
    let admin_reader = BufReader::new(admins_file);
    let admin_wallets: Vec<String> = admin_reader
        .lines()
        .collect::<Result<_, _>>()
        .map_err(|_| ())?;
    let fund_wallets = admin_wallets
        .into_iter()
        .map(|x| Keypair::from_base58_string(&x))
        .collect::<Vec<_>>();

    let config = VolumeHandle {
        sleep_ms_min: 10,
        sleep_ms_max: 200,
        fund_wallets,
        mint: Pubkey::from_str_const("414qoCHkfRMziW7zyJCbabXLWJ2HXV8S8hnNwXispump"),
        creator: Pubkey::from_str_const("CR6RYZWwWJ7GVPjP3NZcpbEEHDN2Y81zVCyKEMYJDXYZ"),
        min_lamports: sol_str_to_lamports("0.05").ok_or(())?,
        max_lamports: sol_str_to_lamports("0.10").ok_or(())?,
        slippage: 500,
        makers,
    };
    handle_volume(config).await.map_err(|_| ())?;
    Ok(())
}

pub async fn handle_volume(config: VolumeHandle) -> Result<(), ()> {
    for admin_wallet in config.fund_wallets.iter().enumerate() {
        println!(
            "Admin wallet {} is {}",
            admin_wallet.0,
            admin_wallet.1.pubkey().to_string()
        );
    }
    let client = SolMut::get_solana_client();
    let mut rng = rand::rng();
    let mut with_token: Vec<Keypair> = Vec::with_capacity(10);

    // let find with tokens

    let mut lamports_found = 0;
    for (index, wallet) in config.makers.iter().enumerate() {
        println!("{}/{}", index, config.makers.len());
        let pubkey = wallet.pubkey();
        let token_account = get_associated_token_address(&pubkey, &config.mint);

        let sol_balance = client.get_balance(&pubkey).await.map_err(|_| ())?;
        lamports_found += sol_balance;
        println!("solbalance {}", sol_balance);
        if sol_balance > sol_str_to_lamports("0.003").ok_or(())? {
            let fund_wallet = match config.fund_wallets.first() {
                Some(wallet) => wallet,
                None => {
                    println!("No fund wallets available!");
                    return Err(());
                }
            };
            let send = system_instruction::transfer(
                &pubkey,
                &fund_wallet.pubkey(),
                sol_balance - sol_str_to_lamports("0.0021").ok_or(())?,
            );
            let build = SolMut::build_versioned_transaction(
                vec![send],
                &wallet.pubkey(),
                client.get_latest_blockhash().await.map_err(|_| ())?,
                &[wallet],
                Vec::with_capacity(0),
            )
            .map_err(|_| ())?;

            let res = client.send_transaction(&build).await.map_err(|_| ())?;
        }
        let Ok(balance) = client.get_token_account_balance(&token_account).await else {
            // println!("Error getting token balance! Maybe not exists!");
            println!("No token found for {}", wallet.pubkey().to_string());
            continue;
        };

        if balance.ui_amount.ok_or(())? > 0.0 {
            println!("{} is with token!", wallet.pubkey().to_string());
            with_token.push(wallet.insecure_clone());
        }
    }

    println!("SOl in wallets : {}", lamports_to_sol(lamports_found));
    'main_loop: loop {
        let recent_blockhash = client.get_latest_blockhash().await.map_err(|_| ())?;
        let is_buy = rand::random_bool(0.0);
        // let maker_wallet = Keypair::from_base58_string(
        //     "5hXWdUXbLDczG5DPehnJwPkAPYYvqua4RhSY1Rmamt24pK9bMvdcmMjGCPJtqysD8m2q9M9S9QpDAoknrU2dq7cW",
        // );

        let maker_wallet = match is_buy {
            true => {
                let Some(maker_wallet) = config.makers.choose(&mut rng) else {
                    println!("mut choose for keypair returned none!");
                    continue;
                };
                maker_wallet.insecure_clone()
            }
            false => {
                let Some(wallet) = with_token.pop() else {
                    println!("No address with tokens to sell!");
                    continue;
                };

                wallet
            }
        };
        println!(
            "Trading with public key : {}",
            maker_wallet.pubkey().to_string()
        );
        let Some(admin_wallet) = config.fund_wallets.choose(&mut rng) else {
            println!("mut choose for keypair returned none!");
            continue;
        };
        let Ok(sol_balance) = client.get_balance(&maker_wallet.pubkey()).await else {
            continue;
        };

        let amount_to_trade = match is_buy {
            true => rand::random_range(config.min_lamports..=config.max_lamports),
            false => {
                let token_account =
                    get_associated_token_address(&maker_wallet.pubkey(), &config.mint);
                let Ok(balance) = client.get_token_account_balance(&token_account).await else {
                    println!("Error getting token balance! Maybe not exists!");
                    continue;
                };

                // sol_str_to_lamports(&balance.ui_amount_string).unwrap_or(0)
                let scaled = balance.ui_amount_string.parse::<f64>().map_err(|_| ())?;
                let scaled = scaled * 10_f64.powf(6.0);
                scaled as u64
            }
        };

        println!("Sol balance : {}", sol_balance);
        println!("amount to trade : {}", amount_to_trade);
        if is_buy {
            if sol_balance < amount_to_trade {
                let outstanding =
                    (amount_to_trade + sol_str_to_lamports("0.0065").ok_or(())?) - sol_balance;
                println!("Outstanding {} lamports", outstanding);
                'admin_loop: for admin_wallet in config.fund_wallets.iter() {
                    let pubkey = admin_wallet.pubkey();
                    let balance_of_admin = client.get_balance(&pubkey).await.map_err(|_| ())?;
                    if balance_of_admin > outstanding {
                        let send = system_instruction::transfer(
                            &admin_wallet.pubkey(),
                            &maker_wallet.pubkey(),
                            outstanding,
                        );
                        let build = SolMut::build_versioned_transaction(
                            vec![send],
                            &admin_wallet.pubkey(),
                            recent_blockhash,
                            &[admin_wallet],
                            Vec::with_capacity(0),
                        )
                        .map_err(|_| ())?;

                        println!(
                            "Sending outstanding SOL of {} to trader",
                            lamports_to_sol(outstanding)
                        );
                        let res = client
                            .send_and_confirm_transaction_with_spinner(&build)
                            .await
                            .map_err(|_| ())?;

                        thread::sleep(Duration::from_secs(5));
                        break 'admin_loop;
                    }
                }
            } else {
                println!("Enough lamports! Proceeding to trade");
            }
        }
        if !is_buy && amount_to_trade <= 0 {
            println!("Nothing to sell lets refresh the loop");
            continue;
        }

        println!("Solana wallet balance is {}", lamports_to_sol(sol_balance));
        let fee =
            Pubkey::from_str("G5UZAVbAf46s7cKWoyKu8kYTip9DGTpbLZ2qa9Aq69dP").map_err(|_| ())?;
        let mut instructions = Vec::new();

        if is_buy {
            let associated = create_associated_token_account_idempotent(
                &maker_wallet.pubkey(),
                &maker_wallet.pubkey(),
                &config.mint,
                &spl_token::ID,
            );
            instructions.push(associated);
        }
        match is_buy {
            true => {
                let curve = PumpFun::get_bonding_curve_account(&client, &config.mint).await;
                if curve.is_err() {
                    continue;
                }
                let curve = curve.map_err(|_| ())?;
                let tokens_to_recv = curve.get_buy_price(amount_to_trade).map_err(|_| ())?;
                let slippage = amount_to_trade * config.slippage / 10000;
                let buy = Buy {
                    amount: tokens_to_recv,
                    max_sol_cost: amount_to_trade + slippage,
                };

                println!("{:?}", buy);
                let trade_ix = pump_fun::buy(
                    &maker_wallet.pubkey(),
                    &config.mint,
                    &fee,
                    &config.creator,
                    buy,
                );

                instructions.push(trade_ix)
            }
            false => {
                let sell_args = pump_fun::Sell {
                    amount: amount_to_trade,
                    min_sol_output: 0,
                };

                println!("{:?}", sell_args);
                let trade_ix = pump_fun::sell(
                    &maker_wallet.pubkey(),
                    &config.mint,
                    &fee,
                    &config.creator,
                    sell_args,
                );
                instructions.push(trade_ix);

                // let token_account =
                //     get_associated_token_address(&maker_wallet.pubkey(), &config.mint);
                // let close_ix = close_account(
                //     &spl_token::ID,
                //     &token_account,
                //     &maker_wallet.pubkey(),
                //     &maker_wallet.pubkey(),
                //     &[&maker_wallet.pubkey()],
                // )
                // instructions.push(close_ix);
            }
        };

        let build = SolMut::build_versioned_transaction(
            instructions,
            &maker_wallet.pubkey(),
            recent_blockhash,
            &[&maker_wallet],
            Vec::with_capacity(0),
        )
        .map_err(|_| ())?;

        let simulate = client.send_transaction(&build).await;
        println!("{:?}", simulate);

        if is_buy {
            with_token.push(maker_wallet.insecure_clone());
        };

        if !is_buy {
            let balance_solana = match client.get_balance(&maker_wallet.pubkey()).await {
                Ok(x) => x,
                Err(_) => todo!(),
            };

            let send = system_instruction::transfer(
                &maker_wallet.pubkey(),
                &admin_wallet.pubkey(),
                balance_solana,
            );
            let build = SolMut::build_versioned_transaction(
                vec![send],
                &maker_wallet.pubkey(),
                recent_blockhash,
                &[&maker_wallet],
                Vec::with_capacity(0),
            )
            .map_err(|_| ())?;
            println!("Sending back solana to admin!");
            let res = client.send_transaction(&build).await.map_err(|_| ())?;
        }

        let random_sleep = rand::random_range(config.sleep_ms_min..=config.sleep_ms_max) as u64;
        thread::sleep(Duration::from_millis(random_sleep));
    }
}
