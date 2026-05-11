//! SAK Binary — CLI daemon.
//! Combines all pillars into one runnable daemon.

use sak_sdk::Kernel;
use sak_core::ChainEvent;
use anyhow::Result;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("Starting SAK daemon...");

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

    let reflex_cfg = kernel.reflex_config();
    let rules_path = kernel.config.rules_path_or_default();

    // Initialize Guardian
    kernel = match kernel.with_guardian(rules_path.as_str()) {
        Ok(k) => k,
        Err(e) => {
            error!("Failed to load Guardian rules: {}", e);
            return Err(e);
        }
    };

    info!(
        rules = %rules_path,
        geyser = ?kernel.config.geyser_endpoint,
        "SAK daemon started — Guardian armed"
    );

    // Spawn Reflex Engine — does not block the Guardian pipeline
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ChainEvent>(256);
    tokio::spawn(async move {
        if let Err(e) = sak_reflex::start(reflex_cfg, tx).await {
            error!("Reflex Engine fatal error: {}", e);
        }
    });

    // Log slot updates to stdout
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let ChainEvent::SlotUpdate { slot, .. } = event {
                info!("SLOT {} — Reflex Engine live", slot);
            }
        }
    });

    // Keep running
    tokio::time::sleep(tokio::time::Duration::from_secs(u64::MAX)).await;

    Ok(())
}
