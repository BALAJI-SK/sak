//! Ika integration client for SAK.
//!
//! Focus: dWallet / MPC policy checks for cross-chain intents before execution.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const IKA_DEFAULT_BASE_URL: &str = "https://devnet-api.ika.xyz";

/// Ika client for dWallet policy and custody checks.
pub struct IkaClient {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

/// Intent to evaluate through Ika custody policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IkaIntent {
    pub source_chain: String,
    pub source_asset: String,
    pub destination_chain: String,
    pub destination_asset: String,
    pub amount: String,
    pub recipient: String,
    pub policy_scope: Option<String>,
}

/// Ika evaluation result used by SAK.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IkaEvaluation {
    pub allow: bool,
    pub reason: String,
    pub policy_id: Option<String>,
    pub risk_score: Option<u32>,
    pub source: String,
}

impl IkaClient {
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
            base_url,
            api_key,
        }
    }

    /// Creates a client if either `IKA_BASE_URL` or `IKA_API_KEY` is present.
    pub fn from_env() -> Option<Self> {
        let base_url = std::env::var("IKA_BASE_URL").unwrap_or_else(|_| IKA_DEFAULT_BASE_URL.into());
        let api_key = std::env::var("IKA_API_KEY").ok().filter(|v| !v.trim().is_empty());
        if std::env::var("IKA_BASE_URL").is_err() && api_key.is_none() {
            return None;
        }
        Some(Self::new(base_url, api_key))
    }

    pub async fn health(&self) -> Result<serde_json::Value> {
        let url = format!("{}/health", self.base_url.trim_end_matches('/'));
        let mut req = self.http.get(url);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }
        let value = req.send().await?.json().await?;
        Ok(value)
    }

    /// Evaluate a custody/interoperability intent via Ika.
    ///
    /// If upstream is unavailable, returns a conservative local fallback.
    pub async fn evaluate_intent(&self, intent: &IkaIntent) -> Result<IkaEvaluation> {
        let url = format!(
            "{}/v1/dwallets/evaluate",
            self.base_url.trim_end_matches('/')
        );

        let mut req = self.http.post(url).json(intent);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                let val: serde_json::Value = resp.json().await.unwrap_or_default();
                let allow = val["allow"].as_bool().unwrap_or(false);
                let reason = val["reason"]
                    .as_str()
                    .unwrap_or("ika_evaluation_completed")
                    .to_string();
                let policy_id = val["policy_id"].as_str().map(str::to_string);
                let risk_score = val["risk_score"].as_u64().map(|v| v as u32);
                Ok(IkaEvaluation {
                    allow,
                    reason,
                    policy_id,
                    risk_score,
                    source: "ika".into(),
                })
            }
            _ => {
                // Conservative fallback: if no policy scope and cross-chain transfer, block.
                let cross_chain = intent.source_chain != intent.destination_chain;
                let allow = !(cross_chain && intent.policy_scope.is_none());
                let reason = if allow {
                    "ika_fallback_allow_low_context".to_string()
                } else {
                    "ika_fallback_reject_missing_policy_scope".to_string()
                };
                Ok(IkaEvaluation {
                    allow,
                    reason,
                    policy_id: None,
                    risk_score: Some(if allow { 40 } else { 75 }),
                    source: "ika_fallback".into(),
                })
            }
        }
    }
}
