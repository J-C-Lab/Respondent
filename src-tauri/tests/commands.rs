use respondent_lib::commands::{end_session, start_session};

#[test]
fn start_session_rejects_empty_title() {
    assert!(start_session(String::new(), "default-output".into()).is_err());
}

#[test]
fn start_session_rejects_empty_output_device() {
    assert!(start_session("Customer call".into(), String::new()).is_err());
}

#[test]
fn start_session_accepts_valid_input() {
    let id = start_session("Customer call".into(), "default-output".into())
        .expect("valid session start");
    assert!(id.starts_with("session-"));
}

#[test]
fn end_session_rejects_empty_id() {
    assert!(end_session(String::new()).is_err());
}

#[test]
fn end_session_accepts_non_empty_id() {
    assert!(end_session("session-123".into()).is_ok());
}
