//! Policy evaluation module.

use crate::relay_core::{RelayError, RelayResult};
use crate::types::{
    AgentIdentity, MetaMap, Payload, RelayJob, SenderIdentity, SessionId, TransportMessage,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct AuthorizationDecision {
    pub allowed: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RoutingDecision {
    pub agent: AgentIdentity,
    pub reason: String,
}

pub trait PolicyEngine: Send + Sync {
    fn authorize(
        &self,
        sender: &SenderIdentity,
        message: &TransportMessage,
    ) -> RelayResult<AuthorizationDecision>;

    fn route(&self, message: &TransportMessage, job: &RelayJob) -> RelayResult<RoutingDecision>;

    fn can_start_job(&self, _session_id: &SessionId) -> RelayResult<bool> {
        Ok(true)
    }
}

#[derive(Debug, Clone)]
pub struct AgentPermission {
    pub allowed_transports: HashSet<String>,
    pub allowed_mime_types: HashSet<String>,
    pub max_file_size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct SecurityPolicyConfig {
    pub allowed_chat_ids: HashSet<String>,
    pub payload_max_bytes: u64,
    pub allowed_mime_types: HashSet<String>,
    pub one_active_job: bool,
    pub per_agent_permissions: HashMap<String, AgentPermission>,
    pub metadata: MetaMap,
}

#[derive(Debug, Clone)]
pub struct SecurityPolicy {
    default_agent: AgentIdentity,
    config: SecurityPolicyConfig,
    session_pins: HashMap<SessionId, AgentIdentity>,
}

impl SecurityPolicy {
    pub fn new(default_agent: AgentIdentity, config: SecurityPolicyConfig) -> Self {
        Self {
            default_agent,
            config,
            session_pins: HashMap::new(),
        }
    }

    pub fn pin_session_agent(&mut self, session_id: SessionId, agent: AgentIdentity) {
        self.session_pins.insert(session_id, agent);
    }

    fn payloads_allowed(&self, message: &TransportMessage) -> RelayResult<()> {
        for payload in &message.payloads {
            match payload {
                Payload::Text(_) | Payload::Link(_) => {}
                Payload::File(file) => {
                    self.check_file_constraints(file.size_bytes, &file.mime_type)?;
                }
                Payload::Zip(zip) => {
                    self.check_file_constraints(zip.file.size_bytes, &zip.file.mime_type)?;
                }
                Payload::Image(image) => {
                    self.check_file_constraints(image.file.size_bytes, &image.file.mime_type)?;
                }
                Payload::Video(video) => {
                    self.check_file_constraints(video.file.size_bytes, &video.file.mime_type)?;
                }
                Payload::Audio(audio) => {
                    self.check_file_constraints(audio.file.size_bytes, &audio.file.mime_type)?;
                }
            }
        }
        Ok(())
    }

    fn check_file_constraints(&self, size_bytes: u64, mime_type: &str) -> RelayResult<()> {
        if size_bytes > self.config.payload_max_bytes {
            return Err(RelayError::Policy(format!(
                "payload exceeds size limit: {} > {}",
                size_bytes, self.config.payload_max_bytes
            )));
        }

        if !self.config.allowed_mime_types.is_empty()
            && !self.config.allowed_mime_types.contains(mime_type)
        {
            return Err(RelayError::Policy(format!(
                "mime type not allowed: {mime_type}"
            )));
        }

        Ok(())
    }
}

impl PolicyEngine for SecurityPolicy {
    fn authorize(
        &self,
        _sender: &SenderIdentity,
        message: &TransportMessage,
    ) -> RelayResult<AuthorizationDecision> {
        if !self
            .config
            .allowed_chat_ids
            .contains(&message.sender.transport.chat_id)
        {
            return Ok(AuthorizationDecision {
                allowed: false,
                reason: Some("chat_id is not in allowlist".to_string()),
            });
        }

        self.payloads_allowed(message)?;

        Ok(AuthorizationDecision {
            allowed: true,
            reason: Some("policy passed".to_string()),
        })
    }

    fn route(&self, message: &TransportMessage, _job: &RelayJob) -> RelayResult<RoutingDecision> {
        if let Some(pinned) = self.session_pins.get(&message.session_id).cloned() {
            return Ok(RoutingDecision {
                agent: pinned,
                reason: "session pinned route".to_string(),
            });
        }

        Ok(RoutingDecision {
            agent: self.default_agent.clone(),
            reason: "default route".to_string(),
        })
    }

    fn can_start_job(&self, _session_id: &SessionId) -> RelayResult<bool> {
        Ok(self.config.one_active_job)
    }
}

#[derive(Debug, Clone)]
pub struct AllowAllPolicy {
    default_agent: AgentIdentity,
    session_pins: HashMap<SessionId, AgentIdentity>,
}

impl AllowAllPolicy {
    pub fn new(default_agent: AgentIdentity) -> Self {
        Self {
            default_agent,
            session_pins: HashMap::new(),
        }
    }

    pub fn pin_session_agent(&mut self, session_id: SessionId, agent: AgentIdentity) {
        self.session_pins.insert(session_id, agent);
    }
}

impl PolicyEngine for AllowAllPolicy {
    fn authorize(
        &self,
        _sender: &SenderIdentity,
        _message: &TransportMessage,
    ) -> RelayResult<AuthorizationDecision> {
        Ok(AuthorizationDecision {
            allowed: true,
            reason: Some("allow-all policy".to_string()),
        })
    }

    fn route(&self, message: &TransportMessage, _job: &RelayJob) -> RelayResult<RoutingDecision> {
        let Some(agent) = self
            .session_pins
            .get(&message.session_id)
            .cloned()
            .or_else(|| Some(self.default_agent.clone()))
        else {
            return Err(RelayError::Policy("no target agent configured".to_string()));
        };

        Ok(RoutingDecision {
            agent,
            reason: "default policy route".to_string(),
        })
    }
}
