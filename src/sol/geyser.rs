use std::{collections::HashMap, time::Duration};

use super::sol_events::sol_platforms::{
    orca::ORCA_ADDRESS, pump_fun::PUMPFUN_ADDRESS, raydium_clmm::RAYDIUM_CLMM_ADDRESS,
};
use futures::Stream;
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcBuilder, GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::{
    geyser::{
        CommitmentLevel, SubscribeRequest, SubscribeRequestFilterTransactions, SubscribeUpdate,
        geyser_client::GeyserClient,
    },
    tonic::Status,
};
pub struct Geyser;
impl Geyser {
    pub async fn build_config() -> Result<GeyserGrpcClient<impl Interceptor>, ()> {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("Failed to install rustls crypto provider");

        let tls_config = ClientTlsConfig::new().with_native_roots();
        let client = GeyserGrpcClient::build_from_shared(
            std::env::var("GEYSER_ENDPOINT")
                .unwrap_or("https://solana-yellowstone-grpc.publicnode.com:443".to_owned()),
        )
        .map_err(|_| ())?
        .tls_config(tls_config)
        .map_err(|_| ())?
        .x_token(std::env::var("GEYSER_X_TOKEN").ok())
        .map_err(|_| ())?
        .connect_timeout(Duration::from_secs(10));

        Ok(client.connect().await.map_err(|_| ())?)
    }
    pub async fn get_stream(
        mut client: GeyserGrpcClient<impl Interceptor>,
    ) -> Result<impl Stream<Item = Result<SubscribeUpdate, Status>>, ()> {
        let mut transactions = HashMap::new();
        let programs_to_listen = vec![
            PUMPFUN_ADDRESS.to_string(),
            ORCA_ADDRESS.to_string(),
            RAYDIUM_CLMM_ADDRESS.to_string(),
            RAYDIUM_CLMM_ADDRESS.to_owned(),
        ];
        transactions.insert(
            "client".to_owned(),
            SubscribeRequestFilterTransactions {
                vote: Some(false),
                failed: Some(false),
                account_include: programs_to_listen, // radium // pump
                account_exclude: vec![],
                account_required: vec![],
                signature: None,
            },
        );

        let subreq = SubscribeRequest {
            accounts: HashMap::default(),
            slots: HashMap::default(),
            transactions,
            transactions_status: HashMap::default(),
            blocks: HashMap::default(),
            blocks_meta: HashMap::default(),
            entry: HashMap::default(),
            commitment: Some(CommitmentLevel::Processed as i32),
            accounts_data_slice: Vec::default(),
            ping: None,
            from_slot: None,
        };

        let (_sink, stream) = client
            .subscribe_with_request(Some(subreq))
            .await
            .map_err(|_| ())?;

        Ok(stream)
    }
}
