use respondent_lib::commands::{
    end_session_for_test, export_session_markdown_for_test, export_session_text_for_test,
    reply_provider_name_for_test, start_session_for_test, SystemStatusEvent,
};
use respondent_lib::session::export::{SessionExport, SessionExportEvent};

#[test]
fn start_session_rejects_empty_title() {
    assert!(start_session_for_test(String::new(), "default-output".into()).is_err());
}

#[test]
fn start_session_rejects_empty_output_device() {
    assert!(start_session_for_test("Customer call".into(), String::new()).is_err());
}

#[test]
fn start_session_accepts_valid_input() {
    let id = start_session_for_test("Customer call".into(), "default-output".into())
        .expect("valid session start");
    assert!(id.starts_with("session-"));
}

#[test]
fn end_session_rejects_empty_id() {
    assert!(end_session_for_test(String::new()).is_err());
}

#[test]
fn end_session_accepts_non_empty_id() {
    assert!(end_session_for_test("session-123".into()).is_ok());
}

#[test]
fn system_status_serializes_to_frontend_contract() {
    let event = SystemStatusEvent::info(Some("s1".to_string()), "ready");
    let value = serde_json::to_value(&event).expect("serialize");
    assert_eq!(value["type"], "system.status");
    assert_eq!(value["sessionId"], "s1");
    assert_eq!(value["level"], "info");
    assert_eq!(value["message"], "ready");
    assert!(value["receivedAtMs"].as_i64().unwrap() > 0);
}

#[test]
fn reply_provider_selection_uses_mock_without_api_key() {
    assert_eq!(reply_provider_name_for_test(None), "mock-llm");
}

#[test]
fn reply_provider_selection_uses_openai_with_api_key() {
    assert_eq!(
        reply_provider_name_for_test(Some("test-key".to_string())),
        "openai-responses-llm"
    );
}

#[test]
fn export_helpers_format_markdown_and_plain_text() {
    let export = SessionExport {
        id: "session-1".into(),
        title: "Meeting".into(),
        started_at: "2026-06-13T00:00:00Z".into(),
        ended_at: Some("2026-06-13T00:01:00Z".into()),
        events: vec![
            SessionExportEvent {
                event_type: "transcript".into(),
                text: "hello".into(),
                is_final: true,
                started_at_ms: 0,
                ended_at_ms: 300,
            },
            SessionExportEvent {
                event_type: "suggestion".into(),
                text: "ask about timing".into(),
                is_final: true,
                started_at_ms: 300,
                ended_at_ms: 300,
            },
        ],
    };

    let markdown = export_session_markdown_for_test(&export);
    let text = export_session_text_for_test(&export);

    assert!(markdown.contains("## Meeting"));
    assert!(markdown.contains("[00:00.300] Suggestion: ask about timing"));
    assert!(text.contains("Transcript: hello"));
}
