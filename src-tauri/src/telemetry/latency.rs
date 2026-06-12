use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyMark {
    pub session_id: String,
    pub metric: String,
    pub value_ms: i64,
    pub recorded_at_ms: i64,
}
