use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ReplyRequest {
    pub session_id: String,
    pub generation_id: String,
    pub transcript: String,
    pub context: Vec<String>,
}

/// Streaming reply events. The wire shape mirrors the frontend RealtimeEvent
/// contract: an internally tagged "type" plus camelCase fields.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum ReplyEvent {
    #[serde(rename = "reply.started")]
    Started {
        session_id: String,
        generation_id: String,
        based_on_transcript_event_id: String,
        received_at_ms: i64,
    },
    #[serde(rename = "reply.token")]
    Token {
        session_id: String,
        generation_id: String,
        token: String,
        received_at_ms: i64,
    },
    #[serde(rename = "reply.final")]
    Final {
        session_id: String,
        generation_id: String,
        text: String,
        received_at_ms: i64,
    },
}

pub trait StreamingReplyClient: Send + Sync {
    fn name(&self) -> &'static str;
}
