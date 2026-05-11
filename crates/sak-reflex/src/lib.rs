//! Pillar 1: Reflex Engine — Yellowstone Geyser subscriber.
//! Subscribes to slot updates via gRPC push streams.
//! Routes typed events to subscribed agent callbacks.

mod config;
mod subscriber;
mod router;

pub use config::ReflexConfig;
pub use subscriber::GeyserSubscriber;
pub use router::EventRouter;
pub use sak_core::{ChainEvent, EventKind};

use anyhow::Result;
use tokio::sync::{broadcast, mpsc};

/// High-level Reflex Engine that combines subscriber + router.
pub struct ReflexEngine {
    subscriber: GeyserSubscriber,
}

impl ReflexEngine {
    pub fn new(endpoint: &str, x_token: Option<String>) -> (Self, EventRouter) {
        let (tx, rx) = broadcast::channel(1024);
        let subscriber = GeyserSubscriber::new(endpoint, x_token, tx);
        let router = EventRouter::new(rx);
        (Self { subscriber }, router)
    }

    pub async fn run(self, filter: SubscribeFilter) -> Result<()> {
        self.subscriber.run(filter).await
    }
}

/// Filter for Geyser subscription.
#[derive(Debug, Clone)]
pub struct SubscribeFilter {
    pub account_pubkeys: Vec<String>,
    pub program_ids: Vec<String>,
}

/// Start the Reflex Engine, sending ChainEvent::SlotUpdate into `tx`.
/// Reconnects automatically on error with 500ms backoff.
pub async fn start(config: ReflexConfig, tx: mpsc::Sender<ChainEvent>) -> Result<()> {
    use futures::StreamExt;
    use std::collections::HashMap;
    use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
    use yellowstone_grpc_proto::prelude::{
        CommitmentLevel, SubscribeRequest, SubscribeRequestFilterSlots,
        subscribe_update::UpdateOneof,
    };

    loop {
        let result: Result<()> = async {
            let mut builder = GeyserGrpcClient::build_from_shared(config.endpoint.clone())?
                .tls_config(ClientTlsConfig::new().with_native_roots())?;

            if !config.token.is_empty() {
                builder = builder.x_token(Some(config.token.as_str()))?;
            }

            let mut client = builder.connect().await?;

            let request = SubscribeRequest {
                slots: HashMap::from([(
                    "slots".to_string(),
                    SubscribeRequestFilterSlots {
                        filter_by_commitment: Some(true),
                        ..Default::default()
                    },
                )]),
                commitment: Some(CommitmentLevel::Processed as i32),
                ..Default::default()
            };

            let mut stream = client.subscribe_once(request).await?;

            tracing::info!("SAK Reflex Engine connected");

            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(update) => {
                        if let Some(UpdateOneof::Slot(slot)) = update.update_oneof {
                            let event = ChainEvent::SlotUpdate {
                                slot: slot.slot,
                                parent: slot.parent,
                                status: slot.status,
                            };
                            if tx.send(event).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(e) => return Err(e.into()),
                }
            }

            Ok(())
        }
        .await;

        if let Err(e) = result {
            tracing::error!("Reflex Engine error: {}, reconnecting in 500ms", e);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}
