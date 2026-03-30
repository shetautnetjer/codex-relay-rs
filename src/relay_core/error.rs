use std::fmt::{Display, Formatter};

pub type RelayResult<T> = Result<T, RelayError>;

#[derive(Debug)]
pub enum RelayError {
    Transport(String),
    Agent(String),
    Artifact(String),
    Policy(String),
    NotFound(String),
    InvalidInput(String),
    Timeout(String),
    Cancelled(String),
    Internal(String),
}

impl Display for RelayError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RelayError::Transport(msg) => write!(f, "transport error: {msg}"),
            RelayError::Agent(msg) => write!(f, "agent error: {msg}"),
            RelayError::Artifact(msg) => write!(f, "artifact error: {msg}"),
            RelayError::Policy(msg) => write!(f, "policy error: {msg}"),
            RelayError::NotFound(msg) => write!(f, "not found: {msg}"),
            RelayError::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
            RelayError::Timeout(msg) => write!(f, "timeout: {msg}"),
            RelayError::Cancelled(msg) => write!(f, "cancelled: {msg}"),
            RelayError::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for RelayError {}
