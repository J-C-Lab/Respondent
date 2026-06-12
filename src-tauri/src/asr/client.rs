use serde::Serialize;

/// Streaming ASR events. The wire shape mirrors the frontend RealtimeEvent
/// contract: an internally tagged "type" plus camelCase fields.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum AsrEvent {
    #[serde(rename = "transcript.partial")]
    Partial {
        session_id: String,
        text: String,
        started_at_ms: i64,
        ended_at_ms: i64,
        received_at_ms: i64,
    },
    #[serde(rename = "transcript.final")]
    Final {
        session_id: String,
        text: String,
        started_at_ms: i64,
        ended_at_ms: i64,
        received_at_ms: i64,
    },
    #[serde(rename = "endpoint.detected")]
    Endpoint {
        session_id: String,
        silence_ms: i64,
        detected_at_ms: i64,
    },
}

pub trait StreamingAsrClient: Send + Sync {
    fn name(&self) -> &'static str;
}
