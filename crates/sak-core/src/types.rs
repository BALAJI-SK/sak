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
    Warn { warnings: Vec<String> },
    Reject { rule: String, reason: String },
}
