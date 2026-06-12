use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SessionExport {
    pub id: String,
    pub title: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub events: Vec<SessionExportEvent>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionExportEvent {
    pub event_type: String,
    pub text: String,
    pub is_final: bool,
    pub started_at_ms: i64,
    pub ended_at_ms: i64,
}
