use crate::relay_core::RelayResult;
use crate::transports::{InMemoryTransport, TransportAdapter, TransportCapabilities};
use crate::types::{
    ArtifactId, CorrelationId, DateTimeLike, FilePayload, MessageId, MetaMap, Payload,
    SenderIdentity, SenderRole, SessionId, TextFormat, TextPayload, TransportIdentity,
    TransportMessage, TransportName, ZipPayload,
};
use std::path::Path;
use std::time::SystemTime;
use uuid::Uuid;

/// Minimal Telegram adapter for v1 local flow:
/// - polling comes from the in-memory transport queue
/// - inbound text updates are normalized into `TransportMessage`
/// - outbound messages are forwarded via normalized `TransportMessage`
#[derive(Debug, Clone)]
pub struct TelegramAdapter {
    inner: InMemoryTransport,
}

impl TelegramAdapter {
    pub fn new() -> Self {
        Self {
            inner: InMemoryTransport::new("telegram"),
        }
    }

    /// Simulates a polled Telegram text update, normalized into the internal schema.
    pub fn enqueue_text_update(
        &self,
        chat_id: impl Into<String>,
        user_id: impl Into<String>,
        text: impl Into<String>,
    ) {
        let chat_id = chat_id.into();
        let user_id = user_id.into();

        let message = TransportMessage {
            message_id: MessageId(Uuid::new_v4().to_string()),
            session_id: SessionId(chat_id.clone()),
            correlation_id: CorrelationId(Uuid::new_v4()),
            occurred_at: DateTimeLike::from(SystemTime::now()),
            sender: Self::sender(chat_id, user_id),
            payloads: vec![Payload::Text(TextPayload {
                text: text.into(),
                format: TextFormat::Plain,
                metadata: MetaMap::new(),
            })],
            reply_to: None,
            tags: vec!["telegram".to_string(), "inbound".to_string()],
            metadata: MetaMap::new(),
        };

        self.inner.push_inbound(message);
    }

    /// Simulates receiving a zip upload from Telegram.
    pub fn enqueue_zip_update(
        &self,
        chat_id: impl Into<String>,
        user_id: impl Into<String>,
        zip_path: impl AsRef<Path>,
    ) {
        let chat_id = chat_id.into();
        let user_id = user_id.into();
        let zip_path = zip_path.as_ref();

        let payload = Payload::Zip(ZipPayload {
            file: FilePayload {
                artifact_id: ArtifactId(Uuid::new_v4()),
                filename: zip_path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| "upload.zip".to_string()),
                mime_type: "application/zip".to_string(),
                size_bytes: std::fs::metadata(zip_path).map(|m| m.len()).unwrap_or(0),
                checksum_sha256: None,
                uri: Some(zip_path.to_string_lossy().to_string()),
                metadata: MetaMap::new(),
            },
            entry_count: None,
            uncompressed_size_bytes: None,
            safe_extracted: None,
            metadata: MetaMap::new(),
        });

        let message = TransportMessage {
            message_id: MessageId(Uuid::new_v4().to_string()),
            session_id: SessionId(chat_id.clone()),
            correlation_id: CorrelationId(Uuid::new_v4()),
            occurred_at: DateTimeLike::from(SystemTime::now()),
            sender: Self::sender(chat_id, user_id),
            payloads: vec![payload],
            reply_to: None,
            tags: vec![
                "telegram".to_string(),
                "inbound".to_string(),
                "zip".to_string(),
            ],
            metadata: MetaMap::new(),
        };

        self.inner.push_inbound(message);
    }

    pub fn sent_messages(&self) -> Vec<TransportMessage> {
        self.inner.outbound_messages()
    }

    fn sender(chat_id: String, user_id: String) -> SenderIdentity {
        SenderIdentity {
            sender_id: user_id.clone(),
            display_name: None,
            role: SenderRole::User,
            transport: TransportIdentity {
                transport: TransportName("telegram".to_string()),
                tenant_id: None,
                chat_id,
                user_id: Some(user_id),
                channel_id: None,
                thread_id: None,
                metadata: MetaMap::new(),
            },
            metadata: MetaMap::new(),
        }
    }
}

impl Default for TelegramAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TransportAdapter for TelegramAdapter {
    fn name(&self) -> &TransportName {
        self.inner.name()
    }

    fn capabilities(&self) -> TransportCapabilities {
        self.inner.capabilities()
    }

    fn poll_inbound(&self) -> RelayResult<Option<TransportMessage>> {
        self.inner.poll_inbound()
    }

    fn send_outbound(&self, message: TransportMessage) -> RelayResult<()> {
        self.inner.send_outbound(message)
    }
}
