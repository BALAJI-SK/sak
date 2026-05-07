//! Pillar 1: Reflex Engine — Yellowstone Geyser subscriber.
//! Subscribes to account deltas via gRPC push streams.
//! Routes typed events to subscribed agent callbacks.

mod subscriber;
mod router;

pub use subscriber::GeyserSubscriber;
pub use router::EventRouter;
pub use sak_core::{ChainEvent, EventKind};

use anyhow::Result;
use tokio::sync::broadcast;

/// High-level Reflex Engine that combines subscriber + router.
pub struct ReflexEngine {
    subscriber: GeyserSubscriber,
    event_tx: broadcast::Sender<ChainEvent>,
}

impl ReflexEngine {
    /// Create a new Reflex Engine.
    /// `endpoint` is the Yellowstone gRPC endpoint (e.g., Helius Geyser URL).
    /// `x_token` is the API key if required (e.g., Helius API key).
    pub fn new(endpoint: &str, x_token: Option<String>) -> (Self, EventRouter) {
        let (tx, rx) = broadcast::channel(1024);
        let subscriber = GeyserSubscriber::new(endpoint, x_token, tx.clone());
        let router = EventRouter::new(rx);
        (
            Self {
                subscriber,
                event_tx: tx,
            },
            router,
        )
    }

    /// Start the Geyser subscriber. This runs indefinitely with reconnect logic.
    /// Provide a filter for which accounts/programs to subscribe to.
    pub async fn run(
        self,
        filter: SubscribeFilter,
    ) -> Result<()> {
        self.subscriber.run(filter).await
    }
}

/// Filter for Geyser subscription.
#[derive(Debug, Clone)]
pub struct SubscribeFilter {
    pub account_pubkeys: Vec<String>,
    pub program_ids: Vec<String>,
}
