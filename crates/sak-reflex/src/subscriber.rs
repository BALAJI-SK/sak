use anyhow::Result;
use tokio::sync::broadcast;
use tracing::{info, warn, error};
use rand::Rng;

use sak_core::{ChainEvent, EventKind};

/// Subscribes to Yellowstone Geyser gRPC stream.
/// Reconnects automatically with exponential backoff.
pub struct GeyserSubscriber {
    endpoint: String,
    x_token: Option<String>,
    event_tx: broadcast::Sender<ChainEvent>,
}

impl GeyserSubscriber {
    pub fn new(
        endpoint: &str,
        x_token: Option<String>,
        event_tx: broadcast::Sender<ChainEvent>,
    ) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            x_token,
            event_tx,
        }
    }

    /// Start the subscriber with reconnect loop.
    pub async fn run(self, _filter: super::SubscribeFilter) -> Result<()> {
        let mut backoff_ms = 100u64;
        let mut rng = rand::thread_rng();

        loop {
            info!("Connecting to Geyser at {}", self.endpoint);

            match self.connect_and_stream().await {
                Ok(()) => {
                    warn!("Geyser stream ended, reconnecting...");
                }
                Err(e) => {
                    error!("Geyser error: {}, reconnecting in {}ms", e, backoff_ms);
                }
            }

            // Exponential backoff with jitter — never fixed sleep
            let jitter = rng.gen_range(0u64..50);
            tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms + jitter)).await;
            backoff_ms = (backoff_ms * 2).min(30_000); // cap at 30s
        }
    }

    async fn connect_and_stream(&self) -> Result<()> {
        // Placeholder: In production, use yellowstone-grpc-client to connect
        // For now, simulate receiving events for demo purposes
        info!("Geyser connected (placeholder — implement Yellowstone gRPC)");

        let mut rng = rand::thread_rng();

        // Simulate receiving an event every 5 seconds
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            let event = ChainEvent {
                slot: rng.gen_range(1000u64..10000),
                kind: EventKind::AccountChanged {
                    pubkey: "11111111111111111111111111111111".into(),
                    lamports: rng.gen_range(1000u64..1000000),
                },
            };

            let _ = self.event_tx.send(event);
        }
    }
}
