use std::fmt;

#[derive(Debug)]
pub enum SakError {
    Geyser(String),
    Simulation(String),
    RuleViolation { rule: String, reason: String },
    State(String),
    Rpc(String),
}

impl fmt::Display for SakError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SakError::Geyser(msg) => write!(f, "Geyser error: {msg}"),
            SakError::Simulation(msg) => write!(f, "Simulation error: {msg}"),
            SakError::RuleViolation { rule, reason } => {
                write!(f, "Rule violation [{rule}]: {reason}")
            }
            SakError::State(msg) => write!(f, "State error: {msg}"),
            SakError::Rpc(msg) => write!(f, "RPC error: {msg}"),
        }
    }
}

impl std::error::Error for SakError {}
