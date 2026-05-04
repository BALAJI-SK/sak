use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct CompressedAccountResult {
    value: Option<CompressedAccountValue>,
}

#[derive(Debug, Deserialize)]
struct CompressedAccountValue {
    items: Vec<Value>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let api_key = std::env::var("HELIUS_API_KEY")
        .context("Set HELIUS_API_KEY to your Helius API key")?;

    let address = std::env::var("WALLET_ADDRESS")
        .context("Set WALLET_ADDRESS to a devnet wallet pubkey to inspect")?;

    let rpc_url = format!("https://devnet.helius-rpc.com/?api-key={}", api_key);

    info!("Throwaway C — Reading compressed accounts via Helius RPC");
    info!(address = %address, "Querying compressed token accounts for wallet");

    let client = reqwest::Client::new();

    // Query compressed token accounts owned by the wallet
    let payload = json!({
        "jsonrpc": "2.0",
        "id": "sak-throwaway-c",
        "method": "getCompressedTokenAccountsByOwner",
        "params": [
            address,
            {},
            { "limit": 10 }
        ]
    });

    let response = client
        .post(&rpc_url)
        .json(&payload)
        .send()
        .await
        .context("RPC request failed")?;

    let body: Value = response.json().await.context("Failed to parse response")?;

    if let Some(error) = body.get("error") {
        warn!(error = ?error, "RPC returned error");
        return Ok(());
    }

    if let Some(result) = body.get("result") {
        if let Some(items) = result.get("value").and_then(|v| v.get("items")).and_then(|v| v.as_array()) {
            info!(count = items.len(), "Compressed token accounts found");
            for (i, item) in items.iter().enumerate() {
                let mint = item.get("mint").and_then(|v| v.as_str()).unwrap_or("unknown");
                let amount = item
                    .get("tokenAmount")
                    .and_then(|v| v.get("uiAmount"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let hash = item.get("hash").and_then(|v| v.as_str()).unwrap_or("unknown");
                info!(
                    index = i,
                    mint,
                    amount,
                    hash,
                    "Compressed token account"
                );
            }

            if items.is_empty() {
                info!("No compressed token accounts found for this wallet on devnet.");
                info!("To test: mint some compressed tokens using the Light Protocol devnet faucet.");
            }
        }
    }

    // Also query compressed SOL accounts
    info!("Querying compressed SOL accounts...");
    let sol_payload = json!({
        "jsonrpc": "2.0",
        "id": "sak-throwaway-c-sol",
        "method": "getCompressedAccountsByOwner",
        "params": [
            address,
            {},
            { "limit": 10 }
        ]
    });

    let sol_response = client
        .post(&rpc_url)
        .json(&sol_payload)
        .send()
        .await
        .context("SOL accounts RPC request failed")?;

    let sol_body: Value = sol_response.json().await?;

    if let Some(result) = sol_body.get("result") {
        if let Some(items) = result.get("value").and_then(|v| v.get("items")).and_then(|v| v.as_array()) {
            info!(count = items.len(), "Compressed SOL accounts found");
            for (i, item) in items.iter().enumerate() {
                let hash = item.get("hash").and_then(|v| v.as_str()).unwrap_or("unknown");
                let lamports = item.get("lamports").and_then(|v| v.as_u64()).unwrap_or(0);
                info!(index = i, hash, lamports, "Compressed account");
            }
        }
    }

    info!("Done. Throwaway C complete.");
    Ok(())
}
