use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

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
    Correct, // 4–5 stars
    Wrong,   // 1–2 stars
    Neutral, // 3 stars
}

/// User feedback submission for a Guardian decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianFeedback {
    pub timestamp: String,
    pub decision: String,
    pub rule: Option<String>,
    pub description: Option<String>,
    pub stars: u8, // 1-5
    #[serde(deserialize_with = "deserialize_feedback_verdict")]
    pub verdict: FeedbackVerdict,
}

/// Accepts serde's `{"Correct":null}` or a plain string (`"correct"` / `"wrong"`).
fn deserialize_feedback_verdict<'de, D>(deserializer: D) -> Result<FeedbackVerdict, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Error as _, MapAccess, Visitor};

    struct VerdictVisitor;
    impl<'de> Visitor<'de> for VerdictVisitor {
        type Value = FeedbackVerdict;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("feedback verdict")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<FeedbackVerdict, E> {
            match v.to_ascii_lowercase().as_str() {
                "correct" => Ok(FeedbackVerdict::Correct),
                "wrong" => Ok(FeedbackVerdict::Wrong),
                "neutral" => Ok(FeedbackVerdict::Neutral),
                _ => Err(E::unknown_variant(v, &["Correct", "Wrong", "Neutral"])),
            }
        }

        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<FeedbackVerdict, M::Error> {
            let (key, _) = map
                .next_entry::<String, serde::de::IgnoredAny>()?
                .ok_or_else(|| M::Error::custom("empty verdict map"))?;
            match key.as_str() {
                "Correct" => Ok(FeedbackVerdict::Correct),
                "Wrong" => Ok(FeedbackVerdict::Wrong),
                "Neutral" => Ok(FeedbackVerdict::Neutral),
                _ => Err(M::Error::unknown_variant(&key, &["Correct", "Wrong", "Neutral"])),
            }
        }
    }

    deserializer.deserialize_any(VerdictVisitor)
}
