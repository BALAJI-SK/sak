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
    pub config: KernelConfig,
}

impl Kernel {
    /// Create a new Kernel shell. Call `with_guardian` before `submit`.
    /// `config` is retained for Reflex/Geyser wiring (`geyser_endpoint`, `helius_api_key`).
    pub fn new(config: KernelConfig) -> Result<Self> {
        Ok(Self {
            guardian: None,
            reflex: None,
            state: None,
            config,
        })
    }

    /// Initialize the Guardian from a rules YAML file.
    pub fn with_guardian(self, rules_path: &str) -> Result<Self> {
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
    /// **Fail-closed:** if Guardian was not configured, returns `Reject` (never `Allow`).
    pub fn submit(&mut self, tx: &VersionedTransaction, meta: &TxMeta) -> Decision {
        match &mut self.guardian {
            Some(g) => g.evaluate(tx, meta),
            None => {
                tracing::error!(
                    "Guardian not initialized — rejecting (configure with with_guardian before submit)"
                );
                Decision::Reject {
                    rule: "sdk_guardian_unconfigured".into(),
                    reason: "Kernel has no Guardian; call with_guardian() before submit()".into(),
                }
            }
        }
    }

    /// Geyser config derived from the kernel config + process environment.
    pub fn reflex_config(&self) -> sak_reflex::ReflexConfig {
        sak_reflex::ReflexConfig::from_kernel_options(
            self.config.geyser_endpoint.clone(),
            self.config.helius_api_key.clone(),
        )
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
#[derive(Clone, Debug, Default)]
pub struct KernelConfig {
    pub geyser_endpoint: Option<String>,
    pub helius_api_key: Option<String>,
    pub rules_path: Option<String>,
}

impl KernelConfig {
    pub fn rules_path_or_default(&self) -> String {
        self.rules_path
            .clone()
            .unwrap_or_else(|| "rules.yaml".into())
    }
}
