use std::collections::HashMap;
use std::fmt;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use crossbeam_channel::{unbounded, Receiver, Sender};
use serde_json::{json, Value};

use crate::audio::convert::{to_pcm16, LinearResampler};
use crate::audio::frame::AudioFrame;

use super::client::{AsrError, AsrEvent, StreamingAsrClient};

const OPENAI_REALTIME_SAMPLE_RATE: u32 = 24_000;

pub trait RealtimeTransport: Send {
    fn send_json(&mut self, value: Value) -> Result<(), AsrError>;
    fn try_recv_json(&mut self) -> Result<Option<Value>, AsrError>;
    fn close(&mut self) -> Result<(), AsrError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptionDelay {
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

impl TranscriptionDelay {
    fn as_str(self) -> &'static str {
        match self {
            TranscriptionDelay::Minimal => "minimal",
            TranscriptionDelay::Low => "low",
            TranscriptionDelay::Medium => "medium",
            TranscriptionDelay::High => "high",
            TranscriptionDelay::XHigh => "xhigh",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct OpenAiRealtimeConfig {
    pub api_key: String,
    pub model: String,
    pub language: Option<String>,
    pub transcription_delay: TranscriptionDelay,
}

impl fmt::Debug for OpenAiRealtimeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiRealtimeConfig")
            .field("api_key", &"<redacted>")
            .field("model", &self.model)
            .field("language", &self.language)
            .field("transcription_delay", &self.transcription_delay)
            .finish()
    }
}

impl OpenAiRealtimeConfig {
    pub fn from_api_key(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "gpt-realtime-whisper".to_string(),
            language: None,
            transcription_delay: TranscriptionDelay::Minimal,
        }
    }
}

pub struct OpenAiRealtimeAsrClient {
    session_id: String,
    config: OpenAiRealtimeConfig,
    transport: Box<dyn RealtimeTransport>,
    sender: Sender<AsrEvent>,
    receiver: Receiver<AsrEvent>,
    item_text: HashMap<String, String>,
    utterance_started_at_ms: Option<i64>,
    last_frame_ended_at_ms: i64,
    resampler: LinearResampler,
}

impl OpenAiRealtimeAsrClient {
    pub fn with_transport(
        session_id: String,
        config: OpenAiRealtimeConfig,
        transport: Box<dyn RealtimeTransport>,
    ) -> Result<Self, AsrError> {
        if config.api_key.trim().is_empty() {
            return Err(AsrError::Provider("missing OPENAI_API_KEY".to_string()));
        }

        let (sender, receiver) = unbounded();
        let mut client = Self {
            session_id,
            config,
            transport,
            sender,
            receiver,
            item_text: HashMap::new(),
            utterance_started_at_ms: None,
            last_frame_ended_at_ms: 0,
            resampler: LinearResampler::new(16_000, OPENAI_REALTIME_SAMPLE_RATE),
        };
        client.send_session_update()?;
        Ok(client)
    }

    fn send_session_update(&mut self) -> Result<(), AsrError> {
        let mut transcription = json!({
            "model": self.config.model,
            "delay": self.config.transcription_delay.as_str(),
        });
        if let Some(language) = &self.config.language {
            transcription["language"] = json!(language);
        }

        self.transport.send_json(json!({
            "type": "session.update",
            "session": {
                "type": "transcription",
                "audio": {
                    "input": {
                        "format": {
                            "type": "audio/pcm",
                            "rate": OPENAI_REALTIME_SAMPLE_RATE,
                        },
                        "transcription": transcription,
                        "turn_detection": Value::Null,
                    },
                },
            },
        }))
    }

    fn drain_provider_events(&mut self) -> Result<(), AsrError> {
        while let Some(event) = self.transport.try_recv_json()? {
            self.handle_provider_event(event)?;
        }
        Ok(())
    }

    fn handle_provider_event(&mut self, event: Value) -> Result<(), AsrError> {
        let event_type = event.get("type").and_then(Value::as_str);
        if event_type == Some("error") {
            return Err(AsrError::Provider("openai realtime error".to_string()));
        }

        // Transcript parsing lands in the next tasks; keep the planned state
        // fields present without changing Task 1's provider-event behavior.
        let _planned_state = (
            &self.session_id,
            &self.sender,
            &self.item_text,
            self.utterance_started_at_ms,
            self.last_frame_ended_at_ms,
        );
        Ok(())
    }

    fn validate_frame(&self, frame: &AudioFrame) -> Result<(), AsrError> {
        if frame.format.sample_rate != 16_000
            || frame.format.channels != 1
            || frame.format.bits_per_sample != 16
        {
            return Err(AsrError::Provider(
                "openai realtime asr expects 16 kHz mono i16 frames".to_string(),
            ));
        }
        Ok(())
    }

    fn encode_frame(&mut self, frame: &AudioFrame) -> String {
        let normalized = frame
            .samples
            .iter()
            .map(|sample| *sample as f32 / i16::MAX as f32)
            .collect::<Vec<_>>();
        let mut resampled = self.resampler.process(&normalized);
        let expected_samples = frame.samples.len() * OPENAI_REALTIME_SAMPLE_RATE as usize
            / frame.format.sample_rate as usize;

        if resampled.len() < expected_samples {
            let pad = normalized.last().copied().unwrap_or_default();
            resampled.resize(expected_samples, pad);
        } else if resampled.len() > expected_samples {
            resampled.truncate(expected_samples);
        }

        let bytes = to_pcm16(&resampled)
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect::<Vec<_>>();
        STANDARD.encode(bytes)
    }
}

impl StreamingAsrClient for OpenAiRealtimeAsrClient {
    fn name(&self) -> &'static str {
        "openai-realtime-asr"
    }

    fn push_frame(&mut self, frame: &AudioFrame) -> Result<(), AsrError> {
        self.validate_frame(frame)?;
        if self.utterance_started_at_ms.is_none() {
            self.utterance_started_at_ms = Some(frame.captured_at_ms as i64);
        }
        self.last_frame_ended_at_ms = frame.captured_at_ms as i64 + frame.duration_ms() as i64;
        let payload = self.encode_frame(frame);
        self.transport.send_json(json!({
            "type": "input_audio_buffer.append",
            "audio": payload,
        }))?;
        self.drain_provider_events()
    }

    fn events(&self) -> Receiver<AsrEvent> {
        self.receiver.clone()
    }

    fn finalize(&mut self) -> Result<(), AsrError> {
        self.transport
            .send_json(json!({"type": "input_audio_buffer.commit"}))?;
        self.drain_provider_events()
    }
}

impl Drop for OpenAiRealtimeAsrClient {
    fn drop(&mut self) {
        let _ = self.transport.close();
    }
}
