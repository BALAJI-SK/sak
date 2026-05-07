use serde::{Deserialize, Serialize};

/// Agent state stored in ZK-compressed accounts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub agent_id: String,
    pub last_n_decisions: Vec<DecisionRecord>,
    pub open_positions: Vec<Position>,
    pub cooldown_until_slot: u64,
    pub violation_history: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub slot: u64,
    pub decision: String,    // "allowed" | "rejected"
    pub rule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub token_mint: String,
    pub amount: u64,
    pub entry_slot: u64,
}
