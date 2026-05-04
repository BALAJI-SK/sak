use anyhow::{Context, Result};
use futures::StreamExt;
use std::collections::HashMap;
use tracing::{error, info};
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::prelude::{
    subscribe_request_filter_accounts_filter::Filter as AccountFilter,
    subscribe_request_filter_accounts_filter_memcmp::Data as MemcmpData,
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterAccounts,
    SubscribeRequestFilterAccountsFilter, SubscribeRequestFilterAccountsFilterMemcmp,
    SubscribeUpdate, subscribe_update::UpdateOneof,
};

// USDC mint on devnet
const USDC_MINT: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";
// SPL Token program
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let endpoint = std::env::var("GEYSER_ENDPOINT")
        .context("Set GEYSER_ENDPOINT to your Helius/Triton Geyser URL")?;
    let token = std::env::var("GEYSER_TOKEN").ok();

    info!("Connecting to Geyser at {}", endpoint);

    let mut client = GeyserGrpcClient::build_from_shared(endpoint)?
        .x_token(token)?
        .connect()
        .await
        .context("Failed to connect to Geyser")?;

    // Filter: all token accounts whose data bytes [0..32] == USDC mint pubkey
    let usdc_mint_bytes = bs58::decode(USDC_MINT)
        .into_vec()
        .context("Invalid USDC mint")?;

    let accounts_filter = SubscribeRequestFilterAccounts {
        account: vec![],
        owner: vec![TOKEN_PROGRAM_ID.to_string()],
        filters: vec![SubscribeRequestFilterAccountsFilter {
            filter: Some(AccountFilter::Memcmp(
                SubscribeRequestFilterAccountsFilterMemcmp {
                    offset: 0,
                    data: Some(MemcmpData::Bytes(usdc_mint_bytes)),
                },
            )),
        }],
        nonempty_txn_signature: None,
    };

    let mut filters = HashMap::new();
    filters.insert("usdc_accounts".to_string(), accounts_filter);

    let request = SubscribeRequest {
        accounts: filters,
        commitment: Some(CommitmentLevel::Processed as i32),
        ..Default::default()
    };

    let (_, mut stream) = client.subscribe_with_request(Some(request)).await?;

    info!("Subscribed — watching USDC token-account changes on devnet");

    while let Some(update) = stream.next().await {
        match update {
            Ok(SubscribeUpdate {
                update_oneof: Some(UpdateOneof::Account(acct_update)),
                ..
            }) => {
                if let Some(acct) = acct_update.account {
                    let pubkey = bs58::encode(&acct.pubkey).into_string();
                    let lamports = acct.lamports;
                    let slot = acct_update.slot;
                    info!(
                        slot,
                        pubkey,
                        lamports,
                        data_len = acct.data.len(),
                        "USDC token account updated"
                    );
                }
            }
            Ok(_) => {}
            Err(e) => {
                error!("Stream error: {e}");
                break;
            }
        }
    }

    Ok(())
}
