//! Relay core module.

mod engine;
mod error;

pub use engine::RelayCore;
pub use error::{RelayError, RelayResult};
