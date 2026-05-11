use anyhow::Result;
use futures::StreamExt;
use std::collections::HashMap;
use tokio::sync::broadcast;
use tracing::{error, info, warn};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::prelude::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterSlots,
    subscribe_update::UpdateOneof,
};

use sak_core::ChainEvent;

/// Subscribes to Yellowstone Geyser gRPC stream via broadcast channel.
/// Reconnects automatically with 500ms backoff.
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

    pub async fn run(self, _filter: super::SubscribeFilter) -> Result<()> {
        loop {
            info!("Connecting to Geyser at {}", self.endpoint);

            match self.connect_and_stream().await {
                Ok(()) => warn!("Geyser stream ended, reconnecting..."),
                Err(e) => error!("Geyser error: {}, reconnecting in 500ms", e),
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    async fn connect_and_stream(&self) -> Result<()> {
        let mut builder = GeyserGrpcClient::build_from_shared(self.endpoint.clone())?
            .tls_config(ClientTlsConfig::new().with_native_roots())?;

        if let Some(token) = &self.x_token {
            if !token.is_empty() {
                builder = builder.x_token(Some(token.as_str()))?;
            }
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

        info!("SAK Reflex Engine connected");

        while let Some(msg) = stream.next().await {
            match msg {
                Ok(update) => {
                    if let Some(UpdateOneof::Slot(slot)) = update.update_oneof {
                        let event = ChainEvent::SlotUpdate {
                            slot: slot.slot,
                            parent: slot.parent,
                            status: slot.status,
                        };
                        let _ = self.event_tx.send(event);
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(())
    }
}
