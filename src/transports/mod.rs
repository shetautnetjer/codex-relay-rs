//! Transport adapter module.

pub mod telegram;

use crate::relay_core::RelayResult;
use crate::types::{MetaMap, TransportMessage, TransportName};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct TransportCapabilities {
    pub supports_threads: bool,
    pub supports_files: bool,
    pub supports_streaming: bool,
    pub max_payload_bytes: Option<u64>,
    pub metadata: MetaMap,
}

pub trait TransportAdapter: Send + Sync {
    fn name(&self) -> &TransportName;
    fn capabilities(&self) -> TransportCapabilities;

    /// Pull next normalized inbound message if available.
    fn poll_inbound(&self) -> RelayResult<Option<TransportMessage>>;

    /// Send normalized outbound message to the transport.
    fn send_outbound(&self, message: TransportMessage) -> RelayResult<()>;
}

#[derive(Debug, Clone)]
pub struct InMemoryTransport {
    name: TransportName,
    inbound: Arc<Mutex<VecDeque<TransportMessage>>>,
    outbound: Arc<Mutex<Vec<TransportMessage>>>,
}

impl InMemoryTransport {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: TransportName(name.into()),
            inbound: Arc::new(Mutex::new(VecDeque::new())),
            outbound: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn push_inbound(&self, message: TransportMessage) {
        if let Ok(mut queue) = self.inbound.lock() {
            queue.push_back(message);
        }
    }

    pub fn outbound_messages(&self) -> Vec<TransportMessage> {
        self.outbound
            .lock()
            .map_or_else(|_| Vec::new(), |items| items.clone())
    }
}

impl TransportAdapter for InMemoryTransport {
    fn name(&self) -> &TransportName {
        &self.name
    }

    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            supports_threads: true,
            supports_files: true,
            supports_streaming: false,
            max_payload_bytes: None,
            metadata: MetaMap::new(),
        }
    }

    fn poll_inbound(&self) -> RelayResult<Option<TransportMessage>> {
        let mut queue = self
            .inbound
            .lock()
            .map_err(|e| crate::relay_core::RelayError::Transport(e.to_string()))?;
        Ok(queue.pop_front())
    }

    fn send_outbound(&self, message: TransportMessage) -> RelayResult<()> {
        let mut queue = self
            .outbound
            .lock()
            .map_err(|e| crate::relay_core::RelayError::Transport(e.to_string()))?;
        queue.push(message);
        Ok(())
    }
}
