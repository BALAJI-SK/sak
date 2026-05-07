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

    /// Load rules from YAML and use an existing LiteSVM instance.
    pub fn from_yaml_with_svm(path: impl AsRef<Path>, svm: litesvm::LiteSVM) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let rules: RuleSet = serde_yaml::from_str(&content)?;
        Ok(Self {
            rules,
            simulator: Simulator::with_svm(svm),
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
            Err(e) => {
                let human_reason = parse_simulation_error(&e);
                Decision::Reject {
                    rule: "pre_sign_simulation".into(),
                    reason: human_reason,
                }
            }
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

/// Parse raw simulation error into human-readable English.
fn parse_simulation_error(err: &str) -> String {
    if err.contains("InsufficientFundsForRent") {
        "Insufficient funds — transaction would leave account below rent minimum".into()
    } else if err.contains("insufficient funds") || err.contains("InsufficientFunds") {
        "Insufficient balance to complete transaction".into()
    } else if err.contains("InvalidAccountData") {
        "Invalid account data — possible wrong token address".into()
    } else if err.contains("ProgramFailedToComplete") {
        "Program execution failed — transaction would revert on-chain".into()
    } else if err.contains("would exceed max") || err.contains("exceeds") {
        "Transaction exceeds account or program limits".into()
    } else {
        "Transaction would fail on-chain — blocked before signing".into()
    }
}
