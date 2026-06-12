use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use respondent_lib::asr::client::{AsrError, StreamingAsrClient};
use respondent_lib::asr::openai_realtime::{
    OpenAiRealtimeAsrClient, OpenAiRealtimeConfig, RealtimeTransport, TranscriptionDelay,
};
use serde_json::{json, Value};

#[derive(Clone, Default)]
struct RecordingHandle {
    sent: Arc<Mutex<Vec<Value>>>,
}

struct RecordingTransport {
    sent: Arc<Mutex<Vec<Value>>>,
    recv: VecDeque<Value>,
}

impl RecordingTransport {
    fn new() -> (RecordingHandle, Self) {
        let handle = RecordingHandle::default();
        (
            handle.clone(),
            Self {
                sent: handle.sent,
                recv: VecDeque::new(),
            },
        )
    }
}

impl RecordingHandle {
    fn sent(&self) -> Vec<Value> {
        self.sent.lock().expect("sent lock").clone()
    }
}

impl RealtimeTransport for RecordingTransport {
    fn send_json(&mut self, value: Value) -> Result<(), AsrError> {
        self.sent.lock().expect("sent lock").push(value);
        Ok(())
    }

    fn try_recv_json(&mut self) -> Result<Option<Value>, AsrError> {
        Ok(self.recv.pop_front())
    }

    fn close(&mut self) -> Result<(), AsrError> {
        Ok(())
    }
}

fn config() -> OpenAiRealtimeConfig {
    OpenAiRealtimeConfig {
        api_key: "test-key".to_string(),
        model: "gpt-realtime-whisper".to_string(),
        language: Some("en".to_string()),
        transcription_delay: TranscriptionDelay::Minimal,
    }
}

#[test]
fn new_sends_transcription_session_update() {
    let (handle, transport) = RecordingTransport::new();

    let client =
        OpenAiRealtimeAsrClient::with_transport("s1".to_string(), config(), Box::new(transport))
            .expect("client");

    assert_eq!(client.name(), "openai-realtime-asr");

    let sent = handle.sent();
    assert_eq!(sent.len(), 1);

    let update = &sent[0];
    assert_eq!(update["type"], "session.update");
    assert_eq!(update["session"]["type"], "transcription");
    assert_eq!(
        update["session"]["audio"]["input"]["format"],
        json!({"type": "audio/pcm", "rate": 24000})
    );
    assert_eq!(
        update["session"]["audio"]["input"]["transcription"]["model"],
        "gpt-realtime-whisper"
    );
    assert_eq!(
        update["session"]["audio"]["input"]["transcription"]["language"],
        "en"
    );
    assert_eq!(
        update["session"]["audio"]["input"]["transcription"]["delay"],
        "minimal"
    );
    assert!(update["session"]["audio"]["input"]["turn_detection"].is_null());
}

#[test]
fn default_config_uses_low_latency_model_and_delay() {
    let config = OpenAiRealtimeConfig::from_api_key("k");

    assert_eq!(config.model, "gpt-realtime-whisper");
    assert_eq!(config.language, None);
    assert_eq!(config.transcription_delay, TranscriptionDelay::Minimal);
}

#[test]
fn whitespace_api_key_is_rejected() {
    let (_, transport) = RecordingTransport::new();

    let result = OpenAiRealtimeAsrClient::with_transport(
        "s1".to_string(),
        OpenAiRealtimeConfig::from_api_key("   "),
        Box::new(transport),
    );

    match result {
        Err(err) => assert_eq!(err.to_string(), "asr provider error: missing OPENAI_API_KEY"),
        Ok(_) => panic!("blank keys should be rejected"),
    }
}
