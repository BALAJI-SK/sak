use serde::{Deserialize, Serialize};

/// A typed on-chain event produced by the Reflex Engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChainEvent {
    AccountChanged { slot: u64, pubkey: String, lamports: u64 },
    ProgramInvoked { slot: u64, program_id: String },
    SlotUpdate { slot: u64, parent: Option<u64>, status: i32 },
    ShredEntry { slot: u64, data: Vec<u8> },
}

/// Legacy slot-scoped event kind (kept for compatibility).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventKind {
    AccountChanged { pubkey: String, lamports: u64 },
    ProgramInvoked { program_id: String },
    SlotUpdate { parent: Option<u64>, status: i32 },
    ShredEntry { data: Vec<u8> },
}

/// The Guardian's verdict on a transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Decision {
    Allow,
    Reject { rule: String, reason: String },
}

/// Intent metadata supplied by the agent alongside the transaction.
/// Rules like slippage_check operate on this rather than raw instruction bytes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TxMeta {
    /// Slippage tolerance declared by the agent, in basis points (1 bps = 0.01%).
    pub slippage_bps: Option<u64>,
    /// Human-readable description of the intended action.
    pub description: Option<String>,
}

/// Feedback verdict derived from user star rating.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackVerdict {
    Correct,   // 4-5 stars
    Wrong,     // 1-2 stars
    Neutral,   // 3 stars
}

/// User feedback submission for a Guardian decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianFeedback {
    pub timestamp: String,
    pub decision: String,
    pub rule: Option<String>,
    pub description: Option<String>,
    pub stars: u8,  // 1-5
    pub verdict: FeedbackVerdict,
}
