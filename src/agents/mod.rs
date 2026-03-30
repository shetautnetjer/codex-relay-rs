//! Agent adapter module.

use crate::relay_core::RelayResult;
use crate::types::{
    AgentExecutionStatus, AgentIdentity, AgentRequest, AgentResponse, DateTimeLike, MetaMap,
    Payload, TextFormat, TextPayload, TransportName,
};
use std::collections::HashMap;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub enum AgentHealth {
    Healthy,
    Degraded(String),
    Unavailable(String),
}

pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> &AgentIdentity;
    fn health(&self) -> RelayResult<AgentHealth>;
    fn execute(&self, request: AgentRequest) -> RelayResult<AgentResponse>;
    fn cancel(&self, job_id: &crate::types::JobId) -> RelayResult<()>;
}

#[derive(Debug, Clone)]
pub struct AgentRegistryEntry {
    pub name: String,
    pub workspace: String,
    pub config_path: String,
    pub allowed_transports: Vec<TransportName>,
    pub capabilities: Vec<String>,
    pub identity: AgentIdentity,
}

#[derive(Debug, Clone, Default)]
pub struct AgentRegistry {
    by_id: HashMap<String, AgentRegistryEntry>,
}

impl AgentRegistry {
    pub fn register(&mut self, entry: AgentRegistryEntry) {
        self.by_id.insert(entry.identity.agent_id.0.clone(), entry);
    }

    pub fn get(&self, agent_id: &str) -> Option<&AgentRegistryEntry> {
        self.by_id.get(agent_id)
    }
}

#[derive(Debug, Clone)]
pub struct NoopAgentAdapter {
    identity: AgentIdentity,
}

impl NoopAgentAdapter {
    pub fn new(identity: AgentIdentity) -> Self {
        Self { identity }
    }
}

impl AgentAdapter for NoopAgentAdapter {
    fn id(&self) -> &AgentIdentity {
        &self.identity
    }

    fn health(&self) -> RelayResult<AgentHealth> {
        Ok(AgentHealth::Healthy)
    }

    fn execute(&self, request: AgentRequest) -> RelayResult<AgentResponse> {
        let now = DateTimeLike::from(SystemTime::now());

        Ok(AgentResponse {
            job_id: request.job_id,
            correlation_id: request.correlation_id,
            agent: request.target_agent,
            status: AgentExecutionStatus::Completed,
            payloads: vec![Payload::Text(TextPayload {
                text: "codex-adapter(v1): request accepted".to_string(),
                format: TextFormat::Plain,
                metadata: MetaMap::new(),
            })],
            error: None,
            started_at: now,
            finished_at: now,
            usage: None,
            metadata: MetaMap::new(),
        })
    }

    fn cancel(&self, _job_id: &crate::types::JobId) -> RelayResult<()> {
        Ok(())
    }
}
