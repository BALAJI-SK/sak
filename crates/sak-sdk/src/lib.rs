//! SAK SDK — Public API for agent developers.
//! This is the high-level interface that agents use to interact with SAK.

use anyhow::Result;
use sak_core::{Decision, TxMeta};
use solana_transaction::versioned::VersionedTransaction;

/// High-level SDK for SAK.
/// Wraps Guardian, Reflex Engine, and ZK State into one interface.
pub struct Kernel {
    guardian: Option<sak_guardian::Guardian>,
    reflex: Option<sak_reflex::ReflexEngine>,
    state: Option<sak_state::ZkState>,
}

impl Kernel {
    /// Create a new Kernel with default configuration.
    pub fn new(_config: KernelConfig) -> Result<Self> {
        Ok(Self {
            guardian: None,
            reflex: None,
            state: None,
        })
    }

    /// Initialize the Guardian from a rules YAML file.
    pub fn with_guardian(mut self, rules_path: &str) -> Result<Self> {
        let guardian = sak_guardian::Guardian::from_yaml(rules_path)?;
        Ok(Self {
            guardian: Some(guardian),
            ..self
        })
    }

    /// Initialize the Reflex Engine (Pillar 1).
    pub fn with_reflex(self, endpoint: &str, x_token: Option<String>) -> (Self, sak_reflex::EventRouter) {
        let (engine, router) = sak_reflex::ReflexEngine::new(endpoint, x_token);
        (
            Self {
                reflex: Some(engine),
                ..self
            },
            router,
        )
    }

    /// Initialize ZK State (Pillar 3).
    pub fn with_state(self) -> Result<Self> {
        let state = sak_state::ZkState::new();
        Ok(Self {
            state: Some(state),
            ..self
        })
    }

    /// Submit a transaction through the Guardian.
    /// Returns Decision after simulation + rule evaluation.
    pub fn submit(&mut self, tx: &VersionedTransaction, meta: &TxMeta) -> Decision {
        match &mut self.guardian {
            Some(g) => g.evaluate(tx, meta),
            None => {
                tracing::warn!("Guardian not initialized, allowing transaction");
                Decision::Allow
            }
        }
    }

    /// Get the state manager.
    pub fn state(&mut self) -> Option<&mut sak_state::ZkState> {
        self.state.as_mut()
    }

    /// Start the Reflex Engine (if initialized).
    pub async fn start_reflex(self, filter: sak_reflex::SubscribeFilter) -> Result<()> {
        if let Some(engine) = self.reflex {
            engine.run(filter).await
        } else {
            tracing::warn!("Reflex Engine not initialized");
            Ok(())
        }
    }
}

/// Configuration for Kernel initialization.
pub struct KernelConfig {
    pub geyser_endpoint: Option<String>,
    pub helius_api_key: Option<String>,
    pub rules_path: Option<String>,
}

impl Default for KernelConfig {
    fn default() -> Self {
        Self {
            geyser_endpoint: None,
            helius_api_key: None,
            rules_path: Some("rules.yaml".into()),
        }
    }
}
