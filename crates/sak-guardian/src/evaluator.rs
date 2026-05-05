use crate::rules::{Rule, RuleSet};
use sak_core::{Decision, TxMeta};

const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";
const COMPUTE_BUDGET_PROGRAM: &str = "ComputeBudget111111111111111111111111111111";

/// A transaction message reduced to the fields the evaluator needs.
/// This decouples the evaluator from the specific Solana SDK version.
pub struct TxView<'a> {
    /// Ordered account keys as base-58 strings.
    pub account_keys: Vec<String>,
    /// Each instruction: (program_id_index, data bytes).
    pub instructions: &'a [(u8, &'a [u8])],
}

pub fn evaluate(rules: &RuleSet, tx: &TxView<'_>, meta: &TxMeta) -> Decision {
    for rule in &rules.rules {
        if let Some((reason, rule_name)) = check_rule(rule, tx, meta) {
            tracing::warn!(rule = %rule_name, reason = %reason, "Guardian blocked transaction");
            return Decision::Reject { rule: rule_name, reason };
        }
    }
    tracing::info!("Guardian approved transaction");
    Decision::Allow
}

fn check_rule(rule: &Rule, tx: &TxView<'_>, meta: &TxMeta) -> Option<(String, String)> {
    match rule {
        Rule::SlippageCheck { name, max_bps } => {
            let bps = meta.slippage_bps?;
            if bps > *max_bps {
                return Some((
                    format!("slippage {}bps exceeds max {}bps", bps, max_bps),
                    name.clone(),
                ));
            }
            None
        }

        Rule::ProgramWhitelist { name, programs } => {
            for (idx, _) in tx.instructions {
                let program_id = &tx.account_keys[*idx as usize];
                if !programs.contains(program_id) {
                    return Some((
                        format!("program {} is not in the whitelist", program_id),
                        name.clone(),
                    ));
                }
            }
            None
        }

        Rule::DrainCheck { name, max_lamports } => {
            for (idx, data) in tx.instructions {
                let program_id = &tx.account_keys[*idx as usize];
                if program_id == SYSTEM_PROGRAM {
                    if let Some(lamports) = parse_system_transfer_lamports(data) {
                        if lamports > *max_lamports {
                            return Some((
                                format!(
                                    "transfer of {} lamports exceeds max {} lamports",
                                    lamports, max_lamports
                                ),
                                name.clone(),
                            ));
                        }
                    }
                }
            }
            None
        }

        Rule::AccountCountCheck { name, max_count } => {
            let count = tx.account_keys.len();
            if count > *max_count {
                return Some((
                    format!("{} accounts exceeds maximum of {}", count, max_count),
                    name.clone(),
                ));
            }
            None
        }

        Rule::ComputeUnitsCheck { name, max_units } => {
            for (idx, data) in tx.instructions {
                let program_id = &tx.account_keys[*idx as usize];
                if program_id == COMPUTE_BUDGET_PROGRAM {
                    if let Some(units) = parse_compute_unit_limit(data) {
                        if units > *max_units {
                            return Some((
                                format!(
                                    "compute unit limit {} exceeds max {}",
                                    units, max_units
                                ),
                                name.clone(),
                            ));
                        }
                    }
                }
            }
            None
        }

        Rule::PriorityFeeCheck { name, max_microlamports } => {
            for (idx, data) in tx.instructions {
                let program_id = &tx.account_keys[*idx as usize];
                if program_id == COMPUTE_BUDGET_PROGRAM {
                    if let Some(price) = parse_compute_unit_price(data) {
                        if price > *max_microlamports {
                            return Some((
                                format!(
                                    "priority fee {} microlamports exceeds max {}",
                                    price, max_microlamports
                                ),
                                name.clone(),
                            ));
                        }
                    }
                }
            }
            None
        }

        Rule::MinTransferLamports { name, min_lamports } => {
            for (idx, data) in tx.instructions {
                let program_id = &tx.account_keys[*idx as usize];
                if program_id == SYSTEM_PROGRAM {
                    if let Some(lamports) = parse_system_transfer_lamports(data) {
                        if lamports < *min_lamports {
                            return Some((
                                format!(
                                    "transfer of {} lamports is below minimum {} lamports",
                                    lamports, min_lamports
                                ),
                                name.clone(),
                            ));
                        }
                    }
                }
            }
            None
        }

        // Stubs — not yet implemented, always pass.
        Rule::ValueCheck { .. } | Rule::DecimalsCheck { .. } => None,
    }
}

// ── Instruction data parsers ──────────────────────────────────────────────────

/// System program instructions are bincode-encoded enums with 4-byte discriminants.
/// Transfer = variant 2 → [0x02, 0x00, 0x00, 0x00, lamports: u64 LE]
/// CreateAccount = variant 0 → [0x00, 0x00, 0x00, 0x00, lamports: u64 LE, ...]
fn parse_system_transfer_lamports(data: &[u8]) -> Option<u64> {
    if data.len() < 12 {
        return None;
    }
    let discriminant = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    match discriminant {
        // Transfer { lamports }
        2 => Some(u64::from_le_bytes([
            data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
        ])),
        // CreateAccount { lamports, space, owner } — also moves lamports
        0 => Some(u64::from_le_bytes([
            data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
        ])),
        _ => None,
    }
}

/// ComputeBudget uses 1-byte discriminants (not bincode).
/// SetComputeUnitLimit = 0x02 → [0x02, units: u32 LE]
fn parse_compute_unit_limit(data: &[u8]) -> Option<u32> {
    if data.len() >= 5 && data[0] == 0x02 {
        Some(u32::from_le_bytes([data[1], data[2], data[3], data[4]]))
    } else {
        None
    }
}

/// SetComputeUnitPrice = 0x03 → [0x03, microlamports: u64 LE]
fn parse_compute_unit_price(data: &[u8]) -> Option<u64> {
    if data.len() >= 9 && data[0] == 0x03 {
        Some(u64::from_le_bytes([
            data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8],
        ]))
    } else {
        None
    }
}
