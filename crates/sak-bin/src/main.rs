//! SAK Binary — CLI daemon.
//! Combines all pillars into one runnable daemon.

use sak_sdk::Kernel;
use sak_core::Decision;
use anyhow::Result;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("Starting SAK daemon...");

    // Initialize Kernel with configuration
    let config = sak_sdk::KernelConfig {
        geyser_endpoint: std::env::var("GEYSER_ENDPOINT").ok(),
        helius_api_key: std::env::var("HELIUS_API_KEY").ok(),
        rules_path: Some("rules.yaml".into()),
    };

    let mut kernel = match Kernel::new(config) {
        Ok(k) => k,
        Err(e) => {
            error!("Failed to initialize Kernel: {}", e);
            return Err(e);
        }
    };

    // Initialize Guardian
    kernel = match kernel.with_guardian("rules.yaml") {
        Ok(k) => k,
        Err(e) => {
            error!("Failed to load Guardian rules: {}", e);
            return Err(e);
        }
    };

    info!("SAK daemon started successfully");
    info!("Guardian loaded with rules from rules.yaml");

    // Keep running (in production, would start Reflex Engine + API server)
    tokio::time::sleep(tokio::time::Duration::from_secs(u64::MAX)).await;

    Ok(())
}
