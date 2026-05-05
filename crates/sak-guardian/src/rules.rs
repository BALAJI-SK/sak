use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSet {
    pub rules: Vec<Rule>,
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
            Rule::DrainCheck { name, .. } => name,
            Rule::AccountCountCheck { name, .. } => name,
            Rule::ComputeUnitsCheck { name, .. } => name,
            Rule::PriorityFeeCheck { name, .. } => name,
            Rule::MinTransferLamports { name, .. } => name,
            Rule::ValueCheck { name, .. } => name,
            Rule::DecimalsCheck { name, .. } => name,
        }
    }
}
