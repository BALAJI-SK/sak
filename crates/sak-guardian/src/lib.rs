mod evaluator;
mod rules;
mod simulator;

pub use rules::{Rule, RuleSet};
use evaluator::{TxView, evaluate};
use simulator::Simulator;

use anyhow::Result;
use sak_core::{Decision, TxMeta};
use solana_transaction::versioned::VersionedTransaction;
use std::path::Path;

pub struct Guardian {
    rules: RuleSet,
    simulator: Simulator,
}

impl Guardian {
    /// Load rules from a YAML file.
    pub fn from_yaml(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let rules: RuleSet = serde_yaml::from_str(&content)?;
        Ok(Self {
            rules,
            simulator: Simulator::new(),
        })
    }

    /// Construct directly from a rule list (useful for tests).
    pub fn with_rules(rules: Vec<Rule>) -> Self {
        Self {
            rules: RuleSet { rules },
            simulator: Simulator::new(),
        }
    }

    /// Simulate and evaluate a full transaction.
    /// Returns Decision after running LiteSVM simulation + rule checks.
    pub fn evaluate(
        &mut self,
        tx: &VersionedTransaction,
        meta: &TxMeta,
    ) -> Decision {
        let sim_result = self.simulator.simulate(tx);
        match sim_result {
            Ok(sim) => {
                // Convert simulation result + original tx to TxView for rule evaluation
                let view = TxView::from_tx_and_sim(tx, &sim);
                evaluate(&self.rules, &view, meta)
            }
            Err(e) => Decision::Reject {
                rule: "pre_sign_simulation".into(),
                reason: format!(
                    "transaction would fail on-chain: {}",
                    e
                ),
            },
        }
    }

    /// Evaluate a transaction represented as account keys + instruction data.
    /// The caller provides account_keys (base-58) and a slice of (program_id_index, data)
    /// pairs, which is exactly what a compiled Solana Message contains.
    pub fn evaluate_raw(
        &self,
        account_keys: Vec<String>,
        instructions: &[(u8, &[u8])],
        meta: &TxMeta,
    ) -> Decision {
        let view = TxView::from_raw(account_keys, instructions);
        evaluate(&self.rules, &view, meta)
    }
}
