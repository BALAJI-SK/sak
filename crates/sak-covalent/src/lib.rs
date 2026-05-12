//! Covalent GoldRush API client for SAK.
//!
//! Provides token verification, wallet balance checks, and transaction history
//! using the GoldRush API (https://goldrush.dev/docs).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const GOLDRUSH_BASE: &str = "https://api.covalenthq.com/v1";

/// Covalent GoldRush API client.
pub struct CovalentClient {
    api_key: String,
    http: reqwest::Client,
}

/// Token balance for a wallet.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenBalance {
    pub contract_address: String,
    pub contract_name: Option<String>,
    pub contract_ticker_symbol: Option<String>,
    pub balance: String,
    pub balance_24h: Option<String>,
    pub quote: Option<f64>,
    pub quote_24h: Option<f64>,
    pub logo_url: Option<String>,
    pub contract_decimals: Option<u32>,
    pub native_token: Option<bool>,
    pub token_type: Option<String>,
}

/// Wallet portfolio summary.
#[derive(Debug, Clone, Deserialize)]
pub struct WalletPortfolio {
    pub address: String,
    pub chain_id: u64,
    pub chain_name: String,
    pub items: Vec<TokenBalance>,
    pub pagination: Option<serde_json::Value>,
}

/// Transaction history item.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransactionItem {
    pub block_signed_at: Option<String>,
    pub tx_hash: String,
    pub successful: Option<bool>,
    pub from_address: String,
    pub to_address: Option<String>,
    pub value: Option<String>,
    pub gas_spent: Option<u64>,
    pub gas_price: Option<u64>,
}

/// Transaction history response.
#[derive(Debug, Clone, Deserialize)]
pub struct TransactionHistory {
    pub address: String,
    pub chain_id: u64,
    pub chain_name: String,
    pub items: Vec<TransactionItem>,
}

/// Token metadata from GoldRush.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenMetadata {
    pub contract_address: String,
    pub contract_name: Option<String>,
    pub contract_ticker_symbol: Option<String>,
    pub contract_decimals: Option<u32>,
    pub logo_url: Option<String>,
    pub quote_rate: Option<f64>,
    pub quote_rate_24h: Option<f64>,
}

impl CovalentClient {
    /// Create a new Covalent client with the given API key.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Create from environment variable `COVALENT_API_KEY`.
    pub fn from_env() -> Option<Self> {
        let key = std::env::var("COVALENT_API_KEY").ok()?;
        if key.is_empty() {
            return None;
        }
        Some(Self::new(key))
    }

    /// Fetch token balances for a wallet on Solana (chain_id = 1399811149).
    pub async fn get_token_balances(&self, address: &str) -> Result<Vec<TokenBalance>> {
        let url = format!(
            "{}/solana-mainnet/address/{}/balances_v2/?key={}",
            GOLDRUSH_BASE, address, self.api_key
        );

        let resp: serde_json::Value = self.http.get(&url).send().await?.json().await?;

        let items = resp["data"]["items"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| serde_json::from_value(item.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(items)
    }

    /// Fetch transaction history for a wallet on Solana.
    pub async fn get_transaction_history(
        &self,
        address: &str,
        limit: Option<u32>,
    ) -> Result<Vec<TransactionItem>> {
        let limit = limit.unwrap_or(10);
        let url = format!(
            "{}/solana-mainnet/address/{}/transactions_v3/?key={}&page-size={}",
            GOLDRUSH_BASE, address, self.api_key, limit
        );

        let resp: serde_json::Value = self.http.get(&url).send().await?.json().await?;

        let items = resp["data"]["items"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| serde_json::from_value(item.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(items)
    }

    /// Get token metadata and pricing for a specific token on Solana.
    pub async fn get_token_metadata(&self, contract_address: &str) -> Result<TokenMetadata> {
        let url = format!(
            "{}/solana-mainnet/tokens/{}/?key={}",
            GOLDRUSH_BASE, contract_address, self.api_key
        );

        let resp: serde_json::Value = self.http.get(&url).send().await?.json().await?;

        let item = &resp["data"]["items"][0];
        if item.is_null() {
            // Token not found in GoldRush — return a minimal stub so callers
            // can treat it as unverified rather than crashing.
            return Ok(TokenMetadata {
                contract_address: contract_address.to_string(),
                contract_name: None,
                contract_ticker_symbol: None,
                contract_decimals: None,
                logo_url: None,
                quote_rate: None,
                quote_rate_24h: None,
            });
        }
        let metadata: TokenMetadata = serde_json::from_value(item.clone())?;

        Ok(metadata)
    }

    /// Check if a token is verified (has name, symbol, decimals, and logo).
    pub async fn is_token_verified(&self, contract_address: &str) -> Result<bool> {
        let meta = self.get_token_metadata(contract_address).await?;
        Ok(meta.contract_name.is_some()
            && meta.contract_ticker_symbol.is_some()
            && meta.contract_decimals.is_some()
            && meta.logo_url.is_some())
    }

    /// Get SOL price in USD from GoldRush.
    pub async fn get_sol_price(&self) -> Result<f64> {
        let url = format!(
            "{}/solana-mainnet/address/So11111111111111111111111111111111111111112/balances_v2/?key={}",
            GOLDRUSH_BASE, self.api_key
        );

        let resp: serde_json::Value = self.http.get(&url).send().await?.json().await?;

        // Extract quote rate for native SOL
        let price = resp["data"]["items"]
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item["quote_rate"].as_f64())
            .unwrap_or(150.0);

        Ok(price)
    }

    /// Check wallet risk based on transaction history patterns.
    /// Returns a risk score from 0 (safe) to 100 (high risk).
    pub async fn assess_wallet_risk(&self, address: &str) -> Result<WalletRiskAssessment> {
        let txs = self.get_transaction_history(address, Some(50)).await?;

        let total_txs = txs.len();
        let failed_txs = txs.iter().filter(|tx| tx.successful == Some(false)).count();
        let unique_contracts: std::collections::HashSet<String> = txs
            .iter()
            .filter_map(|tx| tx.to_address.clone())
            .collect();

        let risk_score = if total_txs == 0 {
            50 // Unknown wallet = medium risk
        } else {
            let failure_rate = failed_txs as f64 / total_txs as f64;
            let contract_diversity = unique_contracts.len() as f64 / total_txs.max(1) as f64;

            // Higher failure rate = higher risk
            // Higher contract diversity = slightly higher risk (could be bot)
            let base_risk = (failure_rate * 60.0) as u32;
            let diversity_penalty = (contract_diversity * 20.0) as u32;

            (base_risk + diversity_penalty).min(100)
        };

        Ok(WalletRiskAssessment {
            address: address.to_string(),
            risk_score,
            total_transactions: total_txs,
            failed_transactions: failed_txs,
            unique_contracts: unique_contracts.len(),
        })
    }
}

/// Wallet risk assessment result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WalletRiskAssessment {
    pub address: String,
    pub risk_score: u32,
    pub total_transactions: usize,
    pub failed_transactions: usize,
    pub unique_contracts: usize,
}
