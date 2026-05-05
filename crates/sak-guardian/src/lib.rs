mod evaluator;
mod rules;

pub use rules::{Rule, RuleSet};
use evaluator::{TxView, evaluate};

use anyhow::Result;
use sak_core::{Decision, TxMeta};
use std::path::Path;

pub struct Guardian {
    rules: RuleSet,
}

impl Guardian {
    /// Load rules from a YAML file.
    pub fn from_yaml(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let rules: RuleSet = serde_yaml::from_str(&content)?;
        Ok(Self { rules })
    }

    /// Construct directly from a rule list (useful for tests).
    pub fn with_rules(rules: Vec<Rule>) -> Self {
        Self { rules: RuleSet { rules } }
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
        let view = TxView { account_keys, instructions };
        evaluate(&self.rules, &view, meta)
    }
}
