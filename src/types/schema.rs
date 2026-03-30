use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::SystemTime;
use uuid::Uuid;

pub type MetaMap = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtifactId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransportName(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderIdentity {
    pub sender_id: String,
    pub display_name: Option<String>,
    pub role: SenderRole,
    pub transport: TransportIdentity,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SenderRole {
    User,
    System,
    Agent,
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportIdentity {
    pub transport: TransportName,
    pub tenant_id: Option<String>,
    pub chat_id: String,
    pub user_id: Option<String>,
    pub channel_id: Option<String>,
    pub thread_id: Option<String>,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub agent_id: AgentId,
    pub display_name: Option<String>,
    pub adapter_kind: String,
    pub workspace_id: String,
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportMessage {
    pub message_id: MessageId,
    pub session_id: SessionId,
    pub correlation_id: CorrelationId,
    pub occurred_at: DateTimeLike,
    pub sender: SenderIdentity,
    pub payloads: Vec<Payload>,
    pub reply_to: Option<MessageId>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayJob {
    pub job_id: JobId,
    pub session_id: SessionId,
    pub correlation_id: CorrelationId,
    pub source_message_id: MessageId,
    pub status: JobStatus,
    pub created_at: DateTimeLike,
    pub updated_at: DateTimeLike,
    pub target_agent: Option<AgentIdentity>,
    pub retry_count: u32,
    pub timeout_ms: Option<u64>,
    pub cancel_requested: bool,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub job_id: JobId,
    pub correlation_id: CorrelationId,
    pub session_id: SessionId,
    pub sender: SenderIdentity,
    pub target_agent: AgentIdentity,
    pub payloads: Vec<Payload>,
    pub instructions: Option<String>,
    pub max_output_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub deadline_unix_ms: Option<u64>,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub job_id: JobId,
    pub correlation_id: CorrelationId,
    pub agent: AgentIdentity,
    pub status: AgentExecutionStatus,
    pub payloads: Vec<Payload>,
    pub error: Option<AgentError>,
    pub started_at: DateTimeLike,
    pub finished_at: DateTimeLike,
    pub usage: Option<UsageStats>,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Payload {
    Text(TextPayload),
    File(FilePayload),
    Zip(ZipPayload),
    Image(ImagePayload),
    Video(VideoPayload),
    Audio(AudioPayload),
    Link(LinkPayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextPayload {
    pub text: String,
    pub format: TextFormat,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextFormat {
    Plain,
    Markdown,
    Html,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePayload {
    pub artifact_id: ArtifactId,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub checksum_sha256: Option<String>,
    pub uri: Option<String>,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZipPayload {
    pub file: FilePayload,
    pub entry_count: Option<u64>,
    pub uncompressed_size_bytes: Option<u64>,
    pub safe_extracted: Option<bool>,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagePayload {
    pub file: FilePayload,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub alt_text: Option<String>,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoPayload {
    pub file: FilePayload,
    pub duration_ms: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f32>,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioPayload {
    pub file: FilePayload,
    pub duration_ms: Option<u64>,
    pub codec: Option<String>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkPayload {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub preview_image: Option<String>,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Received,
    Normalized,
    Authorized,
    Routed,
    Running,
    WaitingRetry,
    Completed,
    Failed,
    TimedOut,
    CancelRequested,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentExecutionStatus {
    Accepted,
    Running,
    Completed,
    Failed,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(default)]
    pub metadata: MetaMap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DateTimeLike {
    pub unix_ms: u64,
}

impl From<SystemTime> for DateTimeLike {
    fn from(value: SystemTime) -> Self {
        let unix_ms = value
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self { unix_ms }
    }
}
