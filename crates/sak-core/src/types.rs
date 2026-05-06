use serde::{Deserialize, Serialize};

/// A slot-stamped on-chain event produced by the Reflex Engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainEvent {
    pub slot: u64,
    pub kind: EventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventKind {
    AccountChanged { pubkey: String, lamports: u64 },
    ProgramInvoked { program_id: String },
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
