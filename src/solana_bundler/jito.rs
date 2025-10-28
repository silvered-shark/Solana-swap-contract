use std::{str::FromStr, thread, time::Duration};

use base64::{Engine, prelude::BASE64_STANDARD};
use jito_sdk_rust::JitoJsonRpcSDK;
use serde::Deserialize;
use serde_json::json;
use solana_sdk::{hash::Hash, pubkey::Pubkey, transaction::VersionedTransaction};

#[derive(Deserialize, Debug)]
pub struct JitoStatusResponse {
    pub result: JitoValue,
}
#[derive(Deserialize, Debug)]
pub struct JitoValueArr {
    pub transactions: Vec<String>,
}
#[derive(Deserialize, Debug)]
pub struct JitoValue {
    pub value: Vec<JitoValueArr>,
}
fn endpoint() -> JitoJsonRpcSDK {
    let jito = jito_sdk_rust::JitoJsonRpcSDK::new(
        &std::env::var("JITO_ENDPOINT")
            .unwrap_or("https://frankfurt.mainnet.block-engine.jito.wtf/api/v1".to_owned()),
        None,
    );
    jito
}
pub async fn jito_get_tip_account() -> Result<Pubkey, ()> {
    let endpoint = endpoint();
    let tip = endpoint
        .get_random_tip_account()
        .await
        .map(|x| Pubkey::from_str(&x).map_err(|x| ()))
        .map_err(|x| ())??;
    Ok(tip)
}
pub async fn bundle(
    blockhash: Hash,
    transactions: Vec<VersionedTransaction>,
) -> Result<String, ()> {
    if transactions.len() > 5 {
        return Err(());
    }
    let jito = endpoint();
    let mut serialized_transactions = Vec::with_capacity(5);
    for tx in transactions {
        let serialized = bincode::serialize(&tx).map_err(|x| ())?;
        let serialized_base64 = BASE64_STANDARD.encode(serialized);
        serialized_transactions.push(serialized_base64);
    }

    let txns = json!(serialized_transactions);
    let params = json!([
        txns,
        {
            "encoding": "base64"
        }
    ]);

    loop {
        let send = jito.send_bundle(Some(params.clone()), None).await;
        println!("{:?}", send);
        match send {
            Ok(x) => {
                let result = x.get("result");

                if result.is_none() {
                    thread::sleep(Duration::from_secs(1));
                    println!("Retrying jito due to no result");
                    continue;
                }
                let result = result.unwrap().as_str();
                if result.is_none() {
                    thread::sleep(Duration::from_secs(1));
                    println!("Retrying jito due to no ids");
                    continue;
                }
                let result = result.unwrap().to_string();

                return Ok(result);
            }

            Err(_) => {
                thread::sleep(Duration::from_secs(1));
                println!("Retrying jito");
                continue;
            }
        };
    }
}

pub async fn check_status(bundle_id: &str) -> Result<JitoStatusResponse, ()> {
    let jito = endpoint();
    let res = jito
        .get_bundle_statuses(vec![bundle_id.to_string()])
        .await
        .map_err(|x| ())?;
    println!("{:#?}", res);
    let parsed = serde_json::from_value::<JitoStatusResponse>(res).map_err(|x| ())?;
    Ok(parsed)
}
