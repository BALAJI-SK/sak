mod evaluator;
mod rules;
mod simulator;

pub use rules::{Rule, RuleIndices, RuleSet};
use evaluator::{TxView, evaluate};
use simulator::Simulator;

use anyhow::Result;
use sak_core::{Decision, TxMeta};
use solana_transaction::versioned::VersionedTransaction;
use std::collections::HashMap;
use std::path::Path;

pub struct Guardian {
    rules: RuleSet,
    simulator: Simulator,
    /// Names of the pack files merged into `rules`, in load order. Empty when
    /// rules were constructed in-memory (e.g. via `with_rules`).
    pack_sources: Vec<String>,
}

/// Summary of the loaded rule set — surfaced to clients via the
/// `/rules/stats` endpoint so the UI doesn't have to fake a count.
#[derive(Debug, Clone)]
pub struct RuleStats {
    pub total: usize,
    pub by_kind: HashMap<&'static str, usize>,
    pub packs: Vec<String>,
}

fn parse_rule_set(content: &str) -> Result<RuleSet> {
    let mut rs: RuleSet = serde_yaml::from_str(content)?;
    rs.rebuild_indices();
    Ok(rs)
}

impl Guardian {
    /// Load rules from a single YAML file.
    pub fn from_yaml(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;
        let rules = parse_rule_set(&content)?;
        Ok(Self {
            rules,
            simulator: Simulator::new(),
            pack_sources: vec![path.display().to_string()],
        })
    }

    /// Load rules from YAML and use an existing LiteSVM instance.
    pub fn from_yaml_with_svm(path: impl AsRef<Path>, svm: litesvm::LiteSVM) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;
        let rules = parse_rule_set(&content)?;
        Ok(Self {
            rules,
            simulator: Simulator::with_svm(svm),
            pack_sources: vec![path.display().to_string()],
        })
    }

    /// Merge multiple in-memory YAML pack contents (e.g. embedded via
    /// `include_str!`) into one Guardian. Each entry is `(name, yaml)` —
    /// `name` is shown in `Guardian::stats().packs` so the UI can list
    /// loaded packs even when nothing lives on disk (Railway, etc.).
    pub fn from_yaml_strings(packs: &[(&str, &str)]) -> Result<Self> {
        let mut merged = RuleSet::default();
        let mut sources: Vec<String> = Vec::new();
        for (name, content) in packs {
            let rs = parse_rule_set(content)?;
            tracing::info!(name = %name, count = rs.len(), "loaded embedded rule pack");
            merged.rules.extend(rs.rules);
            sources.push((*name).to_string());
        }
        merged.rebuild_indices();
        Ok(Self {
            rules: merged,
            simulator: Simulator::new(),
            pack_sources: sources,
        })
    }

    /// Merge multiple YAML pack files into one Guardian. Any non-existent
    /// path is skipped (logged via tracing) so packs can be enabled / disabled
    /// purely by adding or removing files from a directory.
    pub fn from_yaml_files<P: AsRef<Path>>(paths: &[P]) -> Result<Self> {
        let mut merged = RuleSet::default();
        let mut packs: Vec<String> = Vec::new();
        for p in paths {
            let path = p.as_ref();
            if !path.exists() {
                tracing::warn!(path = %path.display(), "rule pack file missing, skipping");
                continue;
            }
            let content = std::fs::read_to_string(path)?;
            let rs = parse_rule_set(&content)?;
            tracing::info!(path = %path.display(), count = rs.len(), "loaded rule pack");
            merged.rules.extend(rs.rules);
            packs.push(path.display().to_string());
        }
        merged.rebuild_indices();
        Ok(Self {
            rules: merged,
            simulator: Simulator::new(),
            pack_sources: packs,
        })
    }

    /// Construct directly from a rule list (useful for tests).
    pub fn with_rules(rules: Vec<Rule>) -> Self {
        Self {
            rules: RuleSet::new(rules),
            simulator: Simulator::new(),
            pack_sources: Vec::new(),
        }
    }

    /// Summary of currently loaded rules — for telemetry & UI display.
    pub fn stats(&self) -> RuleStats {
        let by_kind = self
            .rules
            .count_by_kind()
            .into_iter()
            .collect::<HashMap<_, _>>();
        RuleStats {
            total: self.rules.len(),
            by_kind,
            packs: self.pack_sources.clone(),
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
                match TxView::from_tx_and_sim(tx, &sim) {
                    Ok(view) => evaluate(&self.rules, &view, meta),
                    Err(e) => Decision::Reject {
                        rule: "pre_sign_simulation".into(),
                        reason: e,
                    },
                }
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
    let trimmed = err.trim();
    if trimmed.starts_with("not a legacy tx") {
        "Legacy transaction required — V0 messages not supported".into()
    } else if trimmed.starts_with("InsufficientFundsForRent") {
        "Insufficient funds — transaction would leave account below rent minimum".into()
    } else if trimmed.starts_with("insufficient funds") || trimmed.starts_with("InsufficientFunds") {
        "Insufficient balance to complete transaction".into()
    } else if trimmed.starts_with("InvalidAccountData") {
        "Invalid account data — possible wrong token address".into()
    } else if trimmed.starts_with("ProgramFailedToComplete") {
        "Program execution failed — transaction would revert on-chain".into()
    } else {
        "Transaction would fail on-chain — blocked before signing".into()
    }
}
