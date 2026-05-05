//! Pillar 3: ZK State — Light Protocol compressed accounts.
//! Stores agent state in ZK-compressed accounts (100–1000× cheaper rent).

mod schema;

pub use schema::AgentState;

use anyhow::Result;
use std::collections::HashMap;

/// High-level wrapper for Light Protocol ZK-compressed state.
/// Stores agent state (decisions, positions, cooldowns, violation history).
pub struct ZkState {
    // Placeholder: In production, holds Light Protocol client + Merkle tree context
    // For now, uses in-memory cache (hot state) as described in BUILD_PHASES.md
    cache: HashMap<String, AgentState>,
}

impl ZkState {
    /// Create a new ZK State manager.
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Store agent state.
    /// In production: writes to Light Protocol compressed account.
    /// For now: stores in memory cache (hot state).
    pub fn set(&mut self, agent_id: &str, state: &AgentState) -> Result<()> {
        self.cache.insert(agent_id.to_string(), state.clone());
        tracing::info!(agent_id = %agent_id, "State updated (cached)");
        Ok(())
    }

    /// Read agent state.
    /// In production: reads from ZK compressed account.
    /// For now: reads from memory cache (hot state).
    /// Cold storage (ZK state) flush happens periodically.
    pub fn get(&self, agent_id: &str) -> Result<Option<AgentState>> {
        Ok(self.cache.get(agent_id).cloned())
    }

    /// Flush hot state to ZK compressed accounts (cold storage).
    /// Should be called periodically, NOT in the reflex loop hot path.
    pub fn flush_to_zk(&self) -> Result<()> {
        // Placeholder: In production, batch write to Light Protocol
        tracing::info!("Flushing {} agents to ZK state...", self.cache.len());
        Ok(())
    }
}

/// In-memory cache for hot state (current slot data).
/// Never read ZK state in the critical path of Pillar 1's reflex loop.
pub struct HotCache {
    data: HashMap<String, Vec<u8>>,
}

impl HotCache {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: Vec<u8>) {
        self.data.insert(key.to_string(), value);
    }

    pub fn get(&self, key: &str) -> Option<&Vec<u8>> {
        self.data.get(key)
    }
}
