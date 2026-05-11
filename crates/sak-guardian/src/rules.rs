use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A loaded set of Guardian rules.
///
/// `rules` is the flat list as authored in YAML. `indices` is built once at
/// load time and lets the evaluator dispatch in `O(relevant_rules)` per
/// transaction instead of scanning every rule. This is what makes large
/// blocklist-style packs (thousands of `blocked_program` entries) cheap to
/// evaluate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleSet {
    pub rules: Vec<Rule>,

    /// Indices into `rules`, grouped for fast dispatch. Skipped during
    /// (de)serialization — recomputed via `rebuild_indices()` after load.
    #[serde(skip)]
    pub indices: RuleIndices,
}

/// Pre-computed dispatch tables. Built once per `RuleSet`.
#[derive(Debug, Clone, Default)]
pub struct RuleIndices {
    /// Rules that apply to *every* transaction (global checks).
    pub global: Vec<usize>,
    /// Blocklist rules keyed by program id — only consulted when that
    /// program appears in the tx's account keys.
    pub blocked_program_by_id: HashMap<String, Vec<usize>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Rule {
    /// Reject if agent-declared slippage exceeds the cap.
    #[serde(rename = "slippage_check")]
    SlippageCheck { name: String, max_bps: u64 },

    /// Reject if any instruction invokes a program not in this list.
    #[serde(rename = "program_whitelist")]
    ProgramWhitelist { name: String, programs: Vec<String> },

    /// Reject if the transaction touches a specific program id.
    ///
    /// This is the negative counterpart of `program_whitelist` and is what
    /// blocklist-style rule packs are built from. One rule = one program,
    /// which lets the loader index by program id for O(1) dispatch.
    #[serde(rename = "blocked_program")]
    BlockedProgram { name: String, program: String },

    /// Reject if any system-program Transfer instruction exceeds this amount.
    #[serde(rename = "drain_check")]
    DrainCheck { name: String, max_lamports: u64 },

    /// Reject if the transaction message references more than this many accounts.
    #[serde(rename = "account_count_check")]
    AccountCountCheck { name: String, max_count: usize },

    /// Reject if a ComputeBudget SetComputeUnitLimit instruction exceeds this cap.
    #[serde(rename = "compute_units_check")]
    ComputeUnitsCheck { name: String, max_units: u32 },

    /// Reject if a ComputeBudget SetComputeUnitPrice instruction exceeds this cap.
    #[serde(rename = "priority_fee_check")]
    PriorityFeeCheck { name: String, max_microlamports: u64 },

    /// Reject if any system-program Transfer instruction is below this minimum.
    #[serde(rename = "min_transfer_lamports")]
    MinTransferLamports { name: String, min_lamports: u64 },

    // Stubs — included for YAML schema compatibility, always pass.
    #[serde(rename = "value_check")]
    ValueCheck { name: String, max_usd: f64 },

    #[serde(rename = "decimals_check")]
    DecimalsCheck {
        name: String,
        token: String,
        expected_decimals: u8,
    },
}

impl Rule {
    pub fn name(&self) -> &str {
        match self {
            Rule::SlippageCheck { name, .. } => name,
            Rule::ProgramWhitelist { name, .. } => name,
            Rule::BlockedProgram { name, .. } => name,
            Rule::DrainCheck { name, .. } => name,
            Rule::AccountCountCheck { name, .. } => name,
            Rule::ComputeUnitsCheck { name, .. } => name,
            Rule::PriorityFeeCheck { name, .. } => name,
            Rule::MinTransferLamports { name, .. } => name,
            Rule::ValueCheck { name, .. } => name,
            Rule::DecimalsCheck { name, .. } => name,
        }
    }

    /// Short stable identifier for the rule's kind (used for stats / telemetry).
    pub fn kind(&self) -> &'static str {
        match self {
            Rule::SlippageCheck { .. } => "slippage_check",
            Rule::ProgramWhitelist { .. } => "program_whitelist",
            Rule::BlockedProgram { .. } => "blocked_program",
            Rule::DrainCheck { .. } => "drain_check",
            Rule::AccountCountCheck { .. } => "account_count_check",
            Rule::ComputeUnitsCheck { .. } => "compute_units_check",
            Rule::PriorityFeeCheck { .. } => "priority_fee_check",
            Rule::MinTransferLamports { .. } => "min_transfer_lamports",
            Rule::ValueCheck { .. } => "value_check",
            Rule::DecimalsCheck { .. } => "decimals_check",
        }
    }
}

impl RuleSet {
    /// Construct from a flat rule list and immediately compute dispatch indices.
    pub fn new(rules: Vec<Rule>) -> Self {
        let mut rs = RuleSet { rules, indices: RuleIndices::default() };
        rs.rebuild_indices();
        rs
    }

    /// Append another rule set in place, then rebuild indices once.
    pub fn extend(&mut self, other: RuleSet) {
        self.rules.extend(other.rules);
        self.rebuild_indices();
    }

    /// Rebuild the dispatch indices from `self.rules`. Called automatically
    /// by `new()` / `extend()`; call manually after mutating `rules` directly.
    pub fn rebuild_indices(&mut self) {
        let mut idx = RuleIndices::default();
        for (i, rule) in self.rules.iter().enumerate() {
            match rule {
                Rule::BlockedProgram { program, .. } => {
                    idx.blocked_program_by_id
                        .entry(program.clone())
                        .or_default()
                        .push(i);
                }
                _ => idx.global.push(i),
            }
        }
        self.indices = idx;
    }

    /// Total number of rule instances loaded.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Count rules grouped by `Rule::kind()`. Used by the `/rules/stats` API.
    pub fn count_by_kind(&self) -> HashMap<&'static str, usize> {
        let mut out: HashMap<&'static str, usize> = HashMap::new();
        for r in &self.rules {
            *out.entry(r.kind()).or_insert(0) += 1;
        }
        out
    }
}
