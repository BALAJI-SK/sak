//! Jito bundle submission client for SAK.
//!
//! Submits SAK-approved transactions via Jito Bundle for better execution.
//! Uses Jito's Block Engine API for MEV-protected transaction submission.
//!
//! See: https://docs.jito.wtf (Block Engine, Bundles)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const JITO_BLOCK_ENGINE: &str = "https://mainnet.block-engine.jito.wtf/api/v1";
const JITO_TIP_ACCOUNTS: &[&str] = [
    "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
    "HFqU5x63VTqvQss8hp11i4bVqkfRtQ7NmXwkiLMiXRSE",
    "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
    "ADaUMid9yfUC5e2mR8mT8vH7a4b3Hq3j4t7k6n2p1q9x",
    "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
    "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
    "DttWaMuVvTiDuNwLVS2B95aJGjKGamZiHmXRiCvGMZfE",
    "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
]
.as_slice();

/// Jito bundle submission client.
pub struct JitoClient {
    http: reqwest::Client,
    tip_lamports: u64,
}

/// Bundle submission request.
#[derive(Debug, Serialize)]
struct BundleRequest {
    transactions: Vec<String>,
}

/// Bundle submission response.
#[derive(Debug, Deserialize)]
pub struct BundleResponse {
    pub bundle_id: Option<String>,
    pub error: Option<String>,
}

/// Bundle status.
#[derive(Debug, Deserialize, Serialize)]
pub struct BundleStatus {
    pub bundle_id: String,
    pub status: String,
    pub landed_slot: Option<u64>,
    pub error: Option<String>,
}

/// Result of a Jito bundle submission.
#[derive(Debug, Clone, Serialize)]
pub struct JitoSubmissionResult {
    pub bundle_id: String,
    pub status: String,
    pub tip_lamports: u64,
    pub tip_account: String,
    pub landed_slot: Option<u64>,
    pub error: Option<String>,
}

impl JitoClient {
    /// Create a new Jito client with default tip (10,000 lamports = 0.00001 SOL).
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            tip_lamports: 10_000, // 0.00001 SOL default tip
        }
    }

    /// Create with custom tip amount.
    pub fn with_tip(tip_lamports: u64) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            tip_lamports,
        }
    }

    /// Create from environment variable `JITO_TIP_LAMPORTS`.
    pub fn from_env() -> Self {
        let tip = std::env::var("JITO_TIP_LAMPORTS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10_000);
        Self::with_tip(tip)
    }

    /// Get a random Jito tip account.
    pub fn tip_account(&self) -> &str {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .hash(&mut hasher);
        let idx = hasher.finish() as usize % JITO_TIP_ACCOUNTS.len();
        JITO_TIP_ACCOUNTS[idx]
    }

    /// Submit a bundle of transactions to Jito Block Engine.
    ///
    /// Transactions should be base64-encoded serialized Solana transactions.
    /// The bundle is submitted atomically — either all txs land or none do.
    pub async fn submit_bundle(
        &self,
        transactions: Vec<String>,
    ) -> Result<JitoSubmissionResult> {
        let tip_account = self.tip_account().to_string();

        let request = BundleRequest {
            transactions: transactions.clone(),
        };

        let url = format!("{}/bundles", JITO_BLOCK_ENGINE);

        let resp = self
            .http
            .post(&url)
            .json(&request)
            .send()
            .await?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await?;

        if status.is_success() {
            let bundle_id = body["result"]["bundle_id"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();

            tracing::info!(
                bundle_id = %bundle_id,
                tx_count = transactions.len(),
                tip_lamports = self.tip_lamports,
                tip_account = %tip_account,
                "Jito bundle submitted"
            );

            Ok(JitoSubmissionResult {
                bundle_id,
                status: "submitted".into(),
                tip_lamports: self.tip_lamports,
                tip_account,
                landed_slot: None,
                error: None,
            })
        } else {
            let error = body["error"]["message"]
                .as_str()
                .unwrap_or("Unknown Jito error")
                .to_string();

            tracing::error!(
                status = %status,
                error = %error,
                "Jito bundle submission failed"
            );

            Ok(JitoSubmissionResult {
                bundle_id: String::new(),
                status: "failed".into(),
                tip_lamports: self.tip_lamports,
                tip_account,
                landed_slot: None,
                error: Some(error),
            })
        }
    }

    /// Check the status of a submitted bundle.
    pub async fn get_bundle_status(
        &self,
        bundle_id: &str,
    ) -> Result<BundleStatus> {
        let url = format!("{}/bundles/{}", JITO_BLOCK_ENGINE, bundle_id);

        let resp: serde_json::Value = self.http.get(&url).send().await?.json().await?;

        let status = BundleStatus {
            bundle_id: bundle_id.to_string(),
            status: resp["result"]["status"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            landed_slot: resp["result"]["landed_slot"].as_u64(),
            error: resp["result"]["error"]
                .as_str()
                .map(|s| s.to_string()),
        };

        Ok(status)
    }

    /// Get the tip amount in lamports.
    pub fn tip_lamports(&self) -> u64 {
        self.tip_lamports
    }

    /// Get the tip amount in SOL.
    pub fn tip_sol(&self) -> f64 {
        self.tip_lamports as f64 / 1_000_000_000.0
    }
}

impl Default for JitoClient {
    fn default() -> Self {
        Self::new()
    }
}
