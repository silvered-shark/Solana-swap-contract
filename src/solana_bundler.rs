// WIP

use std::{
    fs::File,
    io::Read,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_sdk::{
    native_token::{sol_str_to_lamports, sol_to_lamports},
    signer::SeedDerivable,
    system_instruction,
};
use solana_signer::{EncodableKey, Signer};
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;

pub mod jito;
use crate::sol::{
    SolMut,
    sol_events::sol_platforms::pump_fun::{
        self, Buy, CreatePumpFun, CreatePumpFunMetadata, GlobalAccount, PUMPFUN,
        create_token_metadata,
    },
};

pub mod pumpfun_bundler;

pub struct AccountToBuy {
    pub keypair: Keypair,
    pub lamport: u64,
}
pub struct PumpFunBundleConfig {
    pub mint_keypair: Keypair,
    pub accounts_to_buy_with: Vec<AccountToBuy>,
}

pub async fn deploy_pump() {
    let config = PumpFunBundleConfig {
        // mint_keypair: Keypair::from_base58_string(
        //     "3DYzXfMZg8U9gorpuQjBw1Spyg5H4oaJXsZ5z4fvQJYp7V5wbAxVKgmi7XsXGmn5orwjXtpPBxzY6ZmcTeUHDTee",
        // ),
        mint_keypair: Keypair::from_base58_string(""),
        accounts_to_buy_with: vec![
            AccountToBuy {
                keypair: Keypair::from_base58_string(""),
                lamport: 34_199_203_154_521, // 1 sol
            },
            AccountToBuy {
                keypair: Keypair::from_base58_string(""),
                lamport: 290_523_943_631_351, // 14 sol
            },
            AccountToBuy {
                keypair: Keypair::from_base58_string(""),
                lamport: 180_198_641_738_146, // 14 sol,
            },
        ],
    };
    deploy_pump_and_buy(config).await;
}
pub async fn deploy_pump_and_buy(config: PumpFunBundleConfig) -> Result<(), ()> {
    // WIP
    let solana = SolMut::get_solana_client();
    let creator_keypair = config.accounts_to_buy_with.first().ok_or(())?;
    println!("Deploying token to {}", config.mint_keypair.pubkey());
    let mut open = File::open("v2.png").map_err(|_| ())?;
    let mut buffer = Vec::new();
    open.read_to_end(&mut buffer).expect("Failed to read file");
    println!(
        "Creator is {}",
        creator_keypair.keypair.pubkey().to_string()
    );

    println!("Address is {}", config.mint_keypair.pubkey().to_string());
    let args = CreatePumpFun {
        name: "".to_string(),
        symbol: "".to_string(),
        uri: "".to_string(),
        creator: creator_keypair.keypair.pubkey(),
    };
    let deploy = crate::sol::sol_events::sol_platforms::pump_fun::create(
        &creator_keypair.keypair,
        &config.mint_keypair,
        args,
    );
    let fee = &Pubkey::from_str("G5UZAVbAf46s7cKWoyKu8kYTip9DGTpbLZ2qa9Aq69dP").map_err(|_| ())?;
    let recent_blockhash = solana.get_latest_blockhash().await.map_err(|_| ())?;
    let buy_args = Buy {
        amount: creator_keypair.lamport,
        max_sol_cost: sol_str_to_lamports("31").ok_or(())?,
    };
    let buy_transaction = pump_fun::buy(
        &creator_keypair.keypair.pubkey(),
        &config.mint_keypair.pubkey(),
        fee,
        &creator_keypair.keypair.pubkey(),
        buy_args,
    );
    let associated = create_associated_token_account_idempotent(
        &creator_keypair.keypair.pubkey(),
        &creator_keypair.keypair.pubkey(),
        &config.mint_keypair.pubkey(),
        &spl_token::ID,
    );
    // lets tip
    let build = SolMut::build_versioned_transaction(
        vec![deploy, associated, buy_transaction],
        &creator_keypair.keypair.pubkey(),
        recent_blockhash,
        &[&creator_keypair.keypair, &config.mint_keypair],
        vec![],
    )
    .map_err(|x| ())
    .map_err(|_| ())?;

    let mut jito_transactions = Vec::new();
    jito_transactions.push(build);

    for (index, buyer) in config.accounts_to_buy_with.iter().skip(1).enumerate() {
        let pubkey = buyer.keypair.pubkey();
        let balance_sol = solana.get_balance(&pubkey).await.unwrap_or(0);
        println!(
            "Buying with address: {} it has a balance of {}",
            buyer.keypair.pubkey().to_string(),
            balance_sol
        );
        let buy_args = Buy {
            amount: buyer.lamport,
            max_sol_cost: sol_str_to_lamports("14.4").ok_or(())?,
        };
        let buy_transaction = pump_fun::buy(
            &buyer.keypair.pubkey(),
            &config.mint_keypair.pubkey(),
            fee,
            &creator_keypair.keypair.pubkey(),
            buy_args,
        );
        let associated = create_associated_token_account_idempotent(
            &buyer.keypair.pubkey(),
            &buyer.keypair.pubkey(),
            &config.mint_keypair.pubkey(),
            &spl_token::ID,
        );

        let mut instructions_to_send = Vec::new();

        instructions_to_send.push(associated);
        instructions_to_send.push(buy_transaction);
        let last_index = config.accounts_to_buy_with.len() - 2;
        if index == last_index {
            let to_tip = system_instruction::transfer(
                &buyer.keypair.pubkey(),
                &jito::jito_get_tip_account().await.map_err(|_| ())?,
                10000,
            );
            instructions_to_send.push(to_tip);
            println!("Pushed tip ");
        }

        let build = SolMut::build_versioned_transaction(
            instructions_to_send,
            &buyer.keypair.pubkey(),
            recent_blockhash,
            &[&buyer.keypair],
            vec![],
        )
        .map_err(|x| ())
        .map_err(|_| ())?;

        jito_transactions.push(build);
    }

    println!("Transactions to send : {}", jito_transactions.len());
    'send_loop: loop {
        let bundle = match jito::bundle(recent_blockhash, jito_transactions.clone()).await {
            Ok(bundle_id) => bundle_id,
            Err(_) => continue,
        };
        loop {
            thread::sleep(Duration::from_secs(5));
            let is_confirmed = jito::check_status(&bundle).await;
            println!("raw: {:?}", is_confirmed);
            match is_confirmed {
                Ok(x) => {
                    println!("{:?}", x);
                }
                Err(_) => break,
            }
        }
        break;
    }

    Ok(())
}

pub fn grind_address(postfix: &str) -> Result<Keypair, ()> {
    let now = Instant::now();
    let threads = thread::available_parallelism().map_err(|_| ())?.get();
    let (tx, rx) = mpsc::channel();
    let postfix = postfix.to_string();

    for _ in 0..threads {
        let tx = tx.clone();
        let postfix = postfix.clone();

        thread::spawn(move || {
            loop {
                let keypair = Keypair::new();
                let address = keypair.pubkey().to_string();

                if address.ends_with(&postfix) {
                    let keypair_pk = keypair.to_base58_string();
                    if tx.send(keypair).is_ok() {
                        println!("MATCH:");
                        println!("{}", keypair_pk);
                        println!("{}", address);
                    }
                    break; // Always break after a match, even if send failed
                }
            }
        });
    }

    // Drop original sender so `rx.recv()` returns if no send happens
    drop(tx);

    // Block until a result is received or all threads exit
    let res = rx.recv().map_err(|_| ());

    println!("Got it in {}", now.elapsed().as_secs());

    res
}
