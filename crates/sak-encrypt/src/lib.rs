//! Encrypt integration client for SAK.
//!
//! Focus: confidential (FHE-style) risk evaluation payloads.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const ENCRYPT_DEFAULT_BASE_URL: &str = "https://devnet-api.encrypt.foundation";

pub struct EncryptClient {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidentialRiskRequest {
    pub wallet: String,
    pub action_type: String,
    pub amount_lamports: u64,
    pub known_counterparty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidentialRiskResult {
    pub ciphertext_id: String,
    pub risk_band: String,
    pub allow: bool,
    pub source: String,
}

impl EncryptClient {
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

    /// Creates a client if either `ENCRYPT_BASE_URL` or `ENCRYPT_API_KEY` is present.
    pub fn from_env() -> Option<Self> {
        let base_url =
            std::env::var("ENCRYPT_BASE_URL").unwrap_or_else(|_| ENCRYPT_DEFAULT_BASE_URL.into());
        let api_key = std::env::var("ENCRYPT_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty());
        if std::env::var("ENCRYPT_BASE_URL").is_err() && api_key.is_none() {
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

    /// Evaluate confidential risk (Encrypt REFHE-style path).
    ///
    /// Falls back to local deterministic confidential tag if upstream is unavailable.
    pub async fn evaluate_confidential_risk(
        &self,
        req_body: &ConfidentialRiskRequest,
    ) -> Result<ConfidentialRiskResult> {
        let url = format!(
            "{}/v1/confidential/risk-evaluate",
            self.base_url.trim_end_matches('/')
        );
        let mut req = self.http.post(url).json(req_body);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                let val: serde_json::Value = resp.json().await.unwrap_or_default();
                Ok(ConfidentialRiskResult {
                    ciphertext_id: val["ciphertext_id"]
                        .as_str()
                        .unwrap_or("enc-unknown")
                        .to_string(),
                    risk_band: val["risk_band"].as_str().unwrap_or("medium").to_string(),
                    allow: val["allow"].as_bool().unwrap_or(false),
                    source: "encrypt".into(),
                })
            }
            _ => {
                let high_amount = req_body.amount_lamports >= 1_000_000_000;
                let unknown_counterparty = !req_body.known_counterparty;
                let high_risk = high_amount && unknown_counterparty;
                let risk_band = if high_risk { "high" } else { "low" };
                Ok(ConfidentialRiskResult {
                    ciphertext_id: format!("enc-fallback-{}", req_body.wallet),
                    risk_band: risk_band.into(),
                    allow: !high_risk,
                    source: "encrypt_fallback".into(),
                })
            }
        }
    }
}
