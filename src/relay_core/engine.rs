use crate::agents::AgentAdapter;
use crate::artifacts::ArtifactFs;
use crate::policy::{AuthorizationDecision, PolicyEngine};
use crate::relay_core::{RelayError, RelayResult};
use crate::transports::TransportAdapter;
use crate::types::{
    AgentRequest, ArtifactId, CorrelationId, DateTimeLike, FilePayload, JobId, JobStatus, MetaMap,
    Payload, RelayJob, TextFormat, TextPayload, TransportMessage, ZipPayload,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Clone)]
pub struct RelayCore<T, A, P>
where
    T: TransportAdapter,
    A: AgentAdapter,
    P: PolicyEngine,
{
    transport: Arc<T>,
    agent: Arc<A>,
    policy: Arc<P>,
    artifacts: Arc<ArtifactFs>,
    active_job_lock: Arc<Mutex<bool>>, // v1: one active job at a time
}

#[derive(Debug, Clone)]
struct ZipContext {
    job_id: String,
    working_dir: PathBuf,
}

impl<T, A, P> RelayCore<T, A, P>
where
    T: TransportAdapter,
    A: AgentAdapter,
    P: PolicyEngine,
{
    pub fn new(
        transport: Arc<T>,
        agent: Arc<A>,
        policy: Arc<P>,
        artifacts: Arc<ArtifactFs>,
    ) -> Self {
        Self {
            transport,
            agent,
            policy,
            artifacts,
            active_job_lock: Arc::new(Mutex::new(false)),
        }
    }

    /// Poll one message and process it end-to-end.
    pub fn poll_once(&self) -> RelayResult<bool> {
        let Some(message) = self.transport.poll_inbound()? else {
            return Ok(false);
        };

        let _guard = ActiveJobGuard::acquire(self.active_job_lock.clone())?;
        self.log("relay.received", &message);

        let auth = self.policy.authorize(&message.sender, &message)?;
        if !auth.allowed {
            self.handle_denied(message, auth)?;
            return Ok(true);
        }

        let now = DateTimeLike::from(SystemTime::now());
        let job_id = JobId(Uuid::new_v4());
        let job = RelayJob {
            job_id: job_id.clone(),
            session_id: message.session_id.clone(),
            correlation_id: message.correlation_id.clone(),
            source_message_id: message.message_id.clone(),
            status: JobStatus::Authorized,
            created_at: now,
            updated_at: now,
            target_agent: None,
            retry_count: 0,
            timeout_ms: Some(60_000),
            cancel_requested: false,
            metadata: MetaMap::new(),
        };

        let zip_ctx = self.detect_and_prepare_zip(&message, &job_id)?;

        let route = self.policy.route(&message, &job)?;
        self.log_simple("relay.routed", &route.reason);

        let mut metadata = MetaMap::new();
        if let Some(ctx) = &zip_ctx {
            metadata.insert(
                "workspace_path".to_string(),
                ctx.working_dir.to_string_lossy().to_string(),
            );
        }

        let request = AgentRequest {
            job_id: job.job_id,
            correlation_id: CorrelationId(Uuid::new_v4()),
            session_id: message.session_id,
            sender: message.sender,
            target_agent: route.agent,
            payloads: message.payloads,
            instructions: None,
            max_output_tokens: Some(2_048),
            temperature: None,
            deadline_unix_ms: Some(now.unix_ms + 60_000),
            metadata,
        };

        match self.agent.execute(request) {
            Ok(response) => {
                self.log_simple("relay.agent_response", "completed");
                let mut response_payloads = response.payloads;

                if let Some(ctx) = zip_ctx {
                    let output_zip_path = self
                        .artifacts
                        .zip_working_to_outbound(&ctx.job_id, "result.zip")?;

                    response_payloads.push(Payload::Zip(ZipPayload {
                        file: FilePayload {
                            artifact_id: ArtifactId(Uuid::new_v4()),
                            filename: "result.zip".to_string(),
                            mime_type: "application/zip".to_string(),
                            size_bytes: std::fs::metadata(&output_zip_path)
                                .map(|m| m.len())
                                .unwrap_or(0),
                            checksum_sha256: None,
                            uri: Some(output_zip_path.to_string_lossy().to_string()),
                            metadata: MetaMap::new(),
                        },
                        entry_count: None,
                        uncompressed_size_bytes: None,
                        safe_extracted: Some(true),
                        metadata: MetaMap::new(),
                    }));
                }

                let outbound = TransportMessage {
                    message_id: crate::types::MessageId(Uuid::new_v4().to_string()),
                    session_id: job.session_id,
                    correlation_id: response.correlation_id,
                    occurred_at: DateTimeLike::from(SystemTime::now()),
                    sender: crate::types::SenderIdentity {
                        sender_id: "relay".to_string(),
                        display_name: Some("codex-relay-rs".to_string()),
                        role: crate::types::SenderRole::System,
                        transport: message.sender.transport,
                        metadata: MetaMap::new(),
                    },
                    payloads: response_payloads,
                    reply_to: Some(job.source_message_id),
                    tags: vec!["reply".to_string()],
                    metadata: MetaMap::new(),
                };
                self.transport.send_outbound(outbound)?;
                Ok(true)
            }
            Err(err) => {
                self.log_simple("relay.agent_error", &err.to_string());
                Err(err)
            }
        }
    }

    fn detect_and_prepare_zip(
        &self,
        message: &TransportMessage,
        job_id: &JobId,
    ) -> RelayResult<Option<ZipContext>> {
        let Some((filename, source_uri)) = find_zip_upload(message) else {
            return Ok(None);
        };

        let source_path = Path::new(&source_uri);
        if !source_path.exists() {
            return Err(RelayError::Artifact(format!(
                "zip source path not found: {}",
                source_path.display()
            )));
        }

        self.log_simple("relay.zip_detected", &filename);
        let job_key = job_id.0.to_string();
        let stored = self
            .artifacts
            .store_inbound_zip(&job_key, source_path, &filename)?;
        let working_dir = self.artifacts.extract_zip_to_working(&job_key, &stored)?;

        Ok(Some(ZipContext {
            job_id: job_key,
            working_dir,
        }))
    }

    fn handle_denied(
        &self,
        message: TransportMessage,
        decision: AuthorizationDecision,
    ) -> RelayResult<()> {
        self.log_simple(
            "relay.denied",
            &decision.reason.unwrap_or_else(|| "not allowed".to_string()),
        );

        let outbound = TransportMessage {
            message_id: crate::types::MessageId(Uuid::new_v4().to_string()),
            session_id: message.session_id,
            correlation_id: message.correlation_id,
            occurred_at: DateTimeLike::from(SystemTime::now()),
            sender: crate::types::SenderIdentity {
                sender_id: "relay".to_string(),
                display_name: Some("codex-relay-rs".to_string()),
                role: crate::types::SenderRole::System,
                transport: message.sender.transport,
                metadata: MetaMap::new(),
            },
            payloads: vec![crate::types::Payload::Text(TextPayload {
                text: "Unauthorized request for this relay.".to_string(),
                format: TextFormat::Plain,
                metadata: MetaMap::new(),
            })],
            reply_to: Some(message.message_id),
            tags: vec!["error".to_string()],
            metadata: MetaMap::new(),
        };

        self.transport.send_outbound(outbound)
    }

    fn log(&self, event: &str, message: &TransportMessage) {
        eprintln!(
            "event={} transport={} session_id={} message_id={}",
            event,
            (message.sender.transport.transport.0),
            message.session_id.0,
            message.message_id.0
        );
    }

    fn log_simple(&self, event: &str, message: &str) {
        eprintln!("event={} detail={}", event, message);
    }
}

fn find_zip_upload(message: &TransportMessage) -> Option<(String, String)> {
    for payload in &message.payloads {
        match payload {
            Payload::Zip(zip) => {
                if let Some(uri) = &zip.file.uri {
                    return Some((zip.file.filename.clone(), uri.clone()));
                }
            }
            Payload::File(file) => {
                if file.mime_type == "application/zip" {
                    if let Some(uri) = &file.uri {
                        return Some((file.filename.clone(), uri.clone()));
                    }
                }
            }
            _ => {}
        }
    }
    None
}

struct ActiveJobGuard {
    lock: Arc<Mutex<bool>>,
}

impl ActiveJobGuard {
    fn acquire(lock: Arc<Mutex<bool>>) -> RelayResult<Self> {
        let mut state = lock
            .lock()
            .map_err(|e| RelayError::Internal(format!("active-job lock failure: {e}")))?;
        if *state {
            return Err(RelayError::Policy(
                "another job is active; v1 allows only one job".to_string(),
            ));
        }
        *state = true;
        drop(state);
        Ok(Self { lock })
    }
}

impl Drop for ActiveJobGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.lock.lock() {
            *state = false;
        }
    }
}
