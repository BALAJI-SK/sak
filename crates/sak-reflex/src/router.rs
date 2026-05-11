use sak_core::ChainEvent;
use tokio::sync::broadcast;

/// Routes events from the Geyser subscriber to agent callbacks.
pub struct EventRouter {
    rx: broadcast::Receiver<ChainEvent>,
}

impl EventRouter {
    pub fn new(rx: broadcast::Receiver<ChainEvent>) -> Self {
        Self { rx }
    }

    /// Subscribe to events. Handler is called for every matching event.
    /// The filter function determines which events to forward.
    pub async fn subscribe<F, Fut>(
        &mut self,
        filter: impl Fn(&ChainEvent) -> bool + Send + Clone + 'static,
        handler: F,
    ) where
        F: Fn(ChainEvent) -> Fut + Send + Clone + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        while let Ok(event) = self.rx.recv().await {
            if filter(&event) {
                let handler = handler.clone();
                tokio::spawn(async move {
                    handler(event).await;
                });
            }
        }
    }

    /// Subscribe to ALL events (convenience method).
    pub async fn subscribe_all<F, Fut>(
        &mut self,
        handler: F,
    ) where
        F: Fn(ChainEvent) -> Fut + Send + Clone + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        self.subscribe(|_| true, handler).await;
    }
}
