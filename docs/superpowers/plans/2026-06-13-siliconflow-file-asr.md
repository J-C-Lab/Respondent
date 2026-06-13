# SiliconFlow File ASR Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Add a file-based SiliconFlow ASR adapter that fits the existing `StreamingAsrClient` trait (buffer in `push_frame`, encode WAV + multipart-POST to `/audio/transcriptions` in `finalize`, emit one `Final`), and generalize ASR provider selection in `commands.rs`.

**Architecture:** No trait or orchestration change — the `TranscriptionSession` already calls `finalize()` at end-of-speech. A pure in-memory WAV encoder + a mockable `TranscriptionTransport` keep the logic deterministically testable; the real transport uses reqwest blocking multipart.

**Tech Stack:** Rust, reqwest (blocking, multipart), serde_json, crossbeam-channel, thiserror. Adds the reqwest `multipart` feature.

---

## File Structure
- `src-tauri/src/asr/siliconflow_file.rs` (create): `encode_wav_pcm16_mono`, `join_transcriptions_url`, `SiliconFlowFileConfig`, `TranscriptionTransport`, `ReqwestTranscriptionTransport`, `SiliconFlowFileAsrClient`.
- `src-tauri/src/asr/mod.rs` (modify): `pub mod siliconflow_file;`.
- `src-tauri/Cargo.toml` (modify): add `multipart` to reqwest features.
- `src-tauri/src/commands.rs` (modify): ASR resolver.
- `src-tauri/tests/siliconflow_file_asr.rs` (create): encoder + client tests.
- `src-tauri/tests/commands.rs` (modify): ASR provider-selection tests.
- `src-tauri/tests/e2e_real_network.rs` (modify): gated transcription smoke.

cargo NOT on default PATH; never `cargo update`:
```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
```

---

## Task 1: WAV Encoder (pure)

**Files:** Create `src-tauri/src/asr/siliconflow_file.rs`; modify `src-tauri/src/asr/mod.rs`; create `src-tauri/tests/siliconflow_file_asr.rs`.

- [ ] **Step 1: Failing encoder tests**

Create `src-tauri/tests/siliconflow_file_asr.rs`:

```rust
use respondent_lib::asr::siliconflow_file::{encode_wav_pcm16_mono, join_transcriptions_url};

#[test]
fn wav_header_and_length_are_correct() {
    let samples = [0x0102i16, -1];
    let wav = encode_wav_pcm16_mono(&samples, 16_000);
    assert_eq!(wav.len(), 44 + samples.len() * 2);
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    assert_eq!(&wav[12..16], b"fmt ");
    assert_eq!(&wav[36..40], b"data");
    assert_eq!(u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]), 16_000);
    assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 1); // channels
    assert_eq!(u16::from_le_bytes([wav[34], wav[35]]), 16); // bits
    assert_eq!(&wav[44..46], &[0x02, 0x01]); // first sample little-endian
}

#[test]
fn wav_empty_samples_is_header_only() {
    let wav = encode_wav_pcm16_mono(&[], 16_000);
    assert_eq!(wav.len(), 44);
    assert_eq!(u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]), 0); // data len
}

#[test]
fn join_transcriptions_url_handles_trailing_slash() {
    assert_eq!(join_transcriptions_url("https://x/v1"), "https://x/v1/audio/transcriptions");
    assert_eq!(join_transcriptions_url("https://x/v1/"), "https://x/v1/audio/transcriptions");
}
```

- [ ] **Step 2: Run RED** — `cargo test --test siliconflow_file_asr` → unresolved import `respondent_lib::asr::siliconflow_file`.

- [ ] **Step 3: Implement the pure functions**

Create `src-tauri/src/asr/siliconflow_file.rs`:

```rust
/// Encode 16-bit mono PCM samples as an in-memory canonical WAV (44-byte
/// header + little-endian i16 data).
pub fn encode_wav_pcm16_mono(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

pub fn join_transcriptions_url(base_url: &str) -> String {
    format!("{}/audio/transcriptions", base_url.trim_end_matches('/'))
}
```

Add to `src-tauri/src/asr/mod.rs`: `pub mod siliconflow_file;`.

- [ ] **Step 4: Run GREEN** — `cargo test --test siliconflow_file_asr` → 3 passed.

- [ ] **Step 5: Commit**
```powershell
git add src-tauri/src/asr/siliconflow_file.rs src-tauri/src/asr/mod.rs src-tauri/tests/siliconflow_file_asr.rs
git commit -m "feat: add wav encoder for file asr"
```

---

## Task 2: File ASR Client + Transport

**Files:** modify `src-tauri/src/asr/siliconflow_file.rs`, `src-tauri/Cargo.toml`, append `src-tauri/tests/siliconflow_file_asr.rs`.

- [ ] **Step 1: Failing client tests** — append to `src-tauri/tests/siliconflow_file_asr.rs`:

```rust
use std::sync::{Arc, Mutex};

use respondent_lib::asr::client::{AsrError, AsrEvent, StreamingAsrClient};
use respondent_lib::asr::siliconflow_file::{
    SiliconFlowFileAsrClient, SiliconFlowFileConfig, TranscriptionTransport,
};
use respondent_lib::audio::frame::{AudioFrame, PcmFormat};

fn config() -> SiliconFlowFileConfig {
    SiliconFlowFileConfig {
        base_url: "https://example.test/v1".into(),
        api_key: "secret-key".into(),
        model: "FunAudioLLM/SenseVoiceSmall".into(),
    }
}

fn frame(amplitude: i16, at_ms: u64) -> AudioFrame {
    AudioFrame {
        format: PcmFormat { sample_rate: 16_000, channels: 1, bits_per_sample: 16 },
        samples: vec![amplitude; 320],
        captured_at_ms: at_ms,
    }
}

struct FakeTransport {
    result: Mutex<Vec<Result<String, AsrError>>>,
    calls: Mutex<usize>,
}
impl FakeTransport {
    fn new(results: Vec<Result<String, AsrError>>) -> Self {
        Self { result: Mutex::new(results), calls: Mutex::new(0) }
    }
}
impl TranscriptionTransport for FakeTransport {
    fn transcribe(&self, _c: &SiliconFlowFileConfig, _wav: &[u8]) -> Result<String, AsrError> {
        *self.calls.lock().unwrap() += 1;
        let mut r = self.result.lock().unwrap();
        if r.is_empty() { Ok(String::new()) } else { r.remove(0) }
    }
}

fn drain(events: &crossbeam_channel::Receiver<AsrEvent>) -> Vec<AsrEvent> {
    let mut out = Vec::new();
    while let Ok(e) = events.try_recv() { out.push(e); }
    out
}

#[test]
fn finalize_uploads_buffer_and_emits_final() {
    let mut client = SiliconFlowFileAsrClient::with_transport(
        "s1".into(), config(),
        Arc::new(FakeTransport::new(vec![Ok("hello world".into())])),
    ).expect("client");
    let events = client.events();
    client.push_frame(&frame(1000, 0)).unwrap();
    client.push_frame(&frame(1000, 20)).unwrap();
    client.finalize().unwrap();
    let drained = drain(&events);
    match drained.as_slice() {
        [AsrEvent::Final { session_id, text, .. }] => {
            assert_eq!(session_id, "s1");
            assert_eq!(text, "hello world");
        }
        other => panic!("expected one final, got {other:?}"),
    }
}

#[test]
fn finalize_without_frames_is_noop() {
    let mut client = SiliconFlowFileAsrClient::with_transport(
        "s1".into(), config(), Arc::new(FakeTransport::new(vec![Ok("x".into())])),
    ).unwrap();
    let events = client.events();
    client.finalize().unwrap();
    assert!(drain(&events).is_empty());
}

#[test]
fn empty_transcript_emits_no_final() {
    let mut client = SiliconFlowFileAsrClient::with_transport(
        "s1".into(), config(), Arc::new(FakeTransport::new(vec![Ok("".into())])),
    ).unwrap();
    let events = client.events();
    client.push_frame(&frame(1000, 0)).unwrap();
    client.finalize().unwrap();
    assert!(drain(&events).is_empty());
}

#[test]
fn transcription_error_does_not_end_session() {
    let mut client = SiliconFlowFileAsrClient::with_transport(
        "s1".into(), config(),
        Arc::new(FakeTransport::new(vec![Err(AsrError::Provider("boom".into())), Ok("second".into())])),
    ).unwrap();
    let events = client.events();
    client.push_frame(&frame(1000, 0)).unwrap();
    client.finalize().unwrap(); // error -> Ok, no final
    assert!(drain(&events).is_empty());
    // buffer cleared; next utterance works
    client.push_frame(&frame(1000, 40)).unwrap();
    client.finalize().unwrap();
    let drained = drain(&events);
    assert!(matches!(drained.as_slice(), [AsrEvent::Final { text, .. }] if text == "second"));
}

#[test]
fn rejects_empty_api_key() {
    let mut cfg = config();
    cfg.api_key = "".into();
    assert!(SiliconFlowFileAsrClient::with_transport(
        "s1".into(), cfg, Arc::new(FakeTransport::new(vec![]))
    ).is_err());
}
```

- [ ] **Step 2: Run RED** — `cargo test --test siliconflow_file_asr` → unresolved `SiliconFlowFileAsrClient` / `TranscriptionTransport`.

- [ ] **Step 3: Add reqwest multipart feature**

In `src-tauri/Cargo.toml`, change the reqwest line to add `multipart`:
```toml
reqwest = { version = "0.12", default-features = false, features = ["blocking", "json", "multipart", "rustls-tls"] }
```

- [ ] **Step 4: Implement config, transport, client**

Append to `src-tauri/src/asr/siliconflow_file.rs`:

```rust
use std::sync::Arc;

use crossbeam_channel::{unbounded, Receiver, Sender};
use serde_json::Value;

use crate::audio::frame::AudioFrame;

use super::client::{AsrError, AsrEvent, StreamingAsrClient};

const TARGET_RATE: u32 = 16_000;

#[derive(Clone)]
pub struct SiliconFlowFileConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

pub trait TranscriptionTransport: Send + Sync {
    fn transcribe(&self, config: &SiliconFlowFileConfig, wav: &[u8]) -> Result<String, AsrError>;
}

fn truncate(text: &str) -> String {
    let t = text.trim();
    if t.len() <= 240 { return t.to_string(); }
    let b = t.char_indices().map(|(i, _)| i).take_while(|i| *i <= 240).last().unwrap_or(0);
    format!("{}...", &t[..b])
}

pub struct ReqwestTranscriptionTransport {
    client: reqwest::blocking::Client,
}
impl Default for ReqwestTranscriptionTransport {
    fn default() -> Self { Self { client: reqwest::blocking::Client::new() } }
}
impl TranscriptionTransport for ReqwestTranscriptionTransport {
    fn transcribe(&self, config: &SiliconFlowFileConfig, wav: &[u8]) -> Result<String, AsrError> {
        let part = reqwest::blocking::multipart::Part::bytes(wav.to_vec())
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| AsrError::Provider(format!("transcription mime: {e}")))?;
        let form = reqwest::blocking::multipart::Form::new()
            .text("model", config.model.clone())
            .part("file", part);
        let response = self
            .client
            .post(join_transcriptions_url(&config.base_url))
            .bearer_auth(&config.api_key)
            .multipart(form)
            .send()
            .map_err(|e| AsrError::Provider(format!("transcription request: {e}")))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(AsrError::Provider(format!("transcription http {status}: {}", truncate(&body))));
        }
        let value: Value = response
            .json()
            .map_err(|e| AsrError::Provider(format!("transcription json: {e}")))?;
        Ok(value["text"].as_str().unwrap_or("").to_string())
    }
}

pub struct SiliconFlowFileAsrClient {
    session_id: String,
    config: SiliconFlowFileConfig,
    transport: Arc<dyn TranscriptionTransport>,
    sender: Sender<AsrEvent>,
    receiver: Receiver<AsrEvent>,
    buffer: Vec<i16>,
    started_at_ms: Option<i64>,
    last_ended_at_ms: i64,
}

impl SiliconFlowFileAsrClient {
    pub fn connect(session_id: String, config: SiliconFlowFileConfig) -> Result<Self, AsrError> {
        Self::with_transport(session_id, config, Arc::new(ReqwestTranscriptionTransport::default()))
    }

    pub fn with_transport(
        session_id: String,
        config: SiliconFlowFileConfig,
        transport: Arc<dyn TranscriptionTransport>,
    ) -> Result<Self, AsrError> {
        if config.api_key.trim().is_empty() {
            return Err(AsrError::Provider("missing SiliconFlow API key".into()));
        }
        if config.base_url.trim().is_empty() {
            return Err(AsrError::Provider("missing SiliconFlow base_url".into()));
        }
        if config.model.trim().is_empty() {
            return Err(AsrError::Provider("missing SiliconFlow ASR model".into()));
        }
        let (sender, receiver) = unbounded();
        Ok(Self {
            session_id,
            config,
            transport,
            sender,
            receiver,
            buffer: Vec::new(),
            started_at_ms: None,
            last_ended_at_ms: 0,
        })
    }
}

impl StreamingAsrClient for SiliconFlowFileAsrClient {
    fn name(&self) -> &'static str {
        "siliconflow-file-asr"
    }

    fn push_frame(&mut self, frame: &AudioFrame) -> Result<(), AsrError> {
        if self.started_at_ms.is_none() {
            self.started_at_ms = Some(frame.captured_at_ms as i64);
        }
        self.last_ended_at_ms = frame.captured_at_ms as i64 + frame.duration_ms() as i64;
        self.buffer.extend_from_slice(&frame.samples);
        Ok(())
    }

    fn events(&self) -> Receiver<AsrEvent> {
        self.receiver.clone()
    }

    fn finalize(&mut self) -> Result<(), AsrError> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let started_at_ms = self.started_at_ms.unwrap_or(0);
        let ended_at_ms = self.last_ended_at_ms;
        let wav = encode_wav_pcm16_mono(&self.buffer, TARGET_RATE);
        self.buffer.clear();
        self.started_at_ms = None;

        match self.transport.transcribe(&self.config, &wav) {
            Ok(text) if !text.trim().is_empty() => {
                let _ = self.sender.send(AsrEvent::Final {
                    session_id: self.session_id.clone(),
                    text,
                    started_at_ms,
                    ended_at_ms,
                    received_at_ms: ended_at_ms,
                });
            }
            Ok(_) => {} // empty transcript -> silent segment
            Err(error) => {
                // One failed segment must not end the session.
                eprintln!("siliconflow transcription failed: {error}");
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 5: Run GREEN** — `cargo test --test siliconflow_file_asr` → 8 passed (3 encoder + 5 client).

- [ ] **Step 6: Commit**
```powershell
git add src-tauri/src/asr/siliconflow_file.rs src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tests/siliconflow_file_asr.rs
git commit -m "feat: add siliconflow file transcription asr client"
```

---

## Task 3: ASR Provider Resolver

**Files:** modify `src-tauri/src/commands.rs`, `src-tauri/tests/commands.rs`.

- [ ] **Step 1: Failing resolver tests** — in `src-tauri/tests/commands.rs`, add (the `env()` HashMap helper already exists from the LLM resolver tests):

```rust
use respondent_lib::commands::resolve_asr_provider_name;

#[test]
fn asr_defaults_to_mock_without_keys() {
    assert_eq!(resolve_asr_provider_name("s1", &env(&[])), "mock-asr");
}

#[test]
fn asr_siliconflow_file_with_key() {
    assert_eq!(
        resolve_asr_provider_name("s1", &env(&[("ASR_PROVIDER", "siliconflow_file"), ("SILICONFLOW_API_KEY", "k")])),
        "siliconflow-file-asr"
    );
}

#[test]
fn asr_siliconflow_file_missing_key_falls_back_to_mock() {
    assert_eq!(
        resolve_asr_provider_name("s1", &env(&[("ASR_PROVIDER", "siliconflow_file")])),
        "mock-asr"
    );
}
```

- [ ] **Step 2: Run RED** — `cargo test --test commands` → unresolved `resolve_asr_provider_name`.

- [ ] **Step 3: Implement the ASR resolver**

In `src-tauri/src/commands.rs`:
1. Add import: `use crate::asr::siliconflow_file::{SiliconFlowFileAsrClient, SiliconFlowFileConfig};`
2. Add the resolver (mirrors `resolve_reply_client`; `get` trims+filters empties from the env map):
```rust
pub fn resolve_asr_client(
    session_id: &str,
    env: &HashMap<String, String>,
) -> Result<(Box<dyn StreamingAsrClient>, bool), String> {
    let provider = env
        .get("ASR_PROVIDER")
        .map(|p| p.trim().to_lowercase())
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "openai_realtime".to_string());
    let get = |key: &str| env.get(key).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());

    match provider.as_str() {
        "siliconflow_file" => match get("SILICONFLOW_API_KEY") {
            Some(api_key) => {
                let config = SiliconFlowFileConfig {
                    base_url: get("SILICONFLOW_BASE_URL")
                        .unwrap_or_else(|| "https://api.siliconflow.cn/v1".to_string()),
                    api_key,
                    model: get("SILICONFLOW_ASR_MODEL")
                        .unwrap_or_else(|| "FunAudioLLM/SenseVoiceSmall".to_string()),
                };
                let client = SiliconFlowFileAsrClient::connect(session_id.to_string(), config)
                    .map_err(|e| e.to_string())?;
                Ok((Box::new(client), false))
            }
            None => Ok((Box::new(MockAsrClient::new(session_id)), true)),
        },
        "openai_realtime" => match get("OPENAI_API_KEY") {
            Some(api_key) => {
                let client = OpenAiRealtimeAsrClient::connect(
                    session_id.to_string(),
                    OpenAiRealtimeConfig::from_api_key(api_key),
                )
                .map_err(|e| e.to_string())?;
                Ok((Box::new(client), false))
            }
            None => Ok((Box::new(MockAsrClient::new(session_id)), true)),
        },
        _ => Ok((Box::new(MockAsrClient::new(session_id)), true)),
    }
}

pub fn resolve_asr_provider_name(session_id: &str, env: &HashMap<String, String>) -> &'static str {
    let (client, _) = resolve_asr_client(session_id, env).expect("resolve asr client");
    client.name()
}
```
3. Replace the body of the existing `build_asr_client(session_id)` to delegate:
```rust
fn build_asr_client(session_id: &str) -> Result<(Box<dyn StreamingAsrClient>, bool), String> {
    resolve_asr_client(session_id, &current_env())
}
```
(`current_env()` already exists from the LLM resolver. Keep the existing `OpenAiRealtimeAsrClient`/`OpenAiRealtimeConfig`/`MockAsrClient` imports.)

- [ ] **Step 4: Run GREEN** — `cargo test --test commands` → all pass (3 new ASR tests + existing).

- [ ] **Step 5: Commit**
```powershell
git add src-tauri/src/commands.rs src-tauri/tests/commands.rs
git commit -m "feat: select asr provider from env (siliconflow file)"
```

---

## Task 4: Full Verification + Gated E2E

**Files:** modify `src-tauri/tests/e2e_real_network.rs`.

- [ ] **Step 1: Gated transcription smoke** — append an `#[ignore]` test reading `SILICONFLOW_API_KEY` that builds `ReqwestTranscriptionTransport`, encodes a short synthetic WAV via `encode_wav_pcm16_mono`, calls `transcribe`, and asserts it returns `Ok` (text may be empty for synthetic audio — only the HTTP round-trip + JSON parse are verified). Read the existing file to match style; import from `respondent_lib::asr::siliconflow_file`.

```rust
#[test]
#[ignore = "uses real SiliconFlow network calls"]
fn real_siliconflow_transcription_roundtrip() {
    use respondent_lib::asr::siliconflow_file::{
        encode_wav_pcm16_mono, ReqwestTranscriptionTransport, SiliconFlowFileConfig,
        TranscriptionTransport,
    };
    let Some(api_key) = std::env::var("SILICONFLOW_API_KEY").ok().filter(|v| !v.trim().is_empty()) else {
        eprintln!("skipping: SILICONFLOW_API_KEY not set");
        return;
    };
    let samples: Vec<i16> = (0..16_000)
        .map(|i| ((i as f32 / 16_000.0 * 440.0 * std::f32::consts::TAU).sin() * 4000.0) as i16)
        .collect();
    let wav = encode_wav_pcm16_mono(&samples, 16_000);
    let transport = ReqwestTranscriptionTransport::default();
    let config = SiliconFlowFileConfig {
        base_url: "https://api.siliconflow.cn/v1".into(),
        api_key,
        model: std::env::var("SILICONFLOW_ASR_MODEL").ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "FunAudioLLM/SenseVoiceSmall".to_string()),
    };
    let text = transport.transcribe(&config, &wav).expect("transcription round-trip");
    eprintln!("[siliconflow-asr] text = {text:?}");
}
```

- [ ] **Step 2: Full Rust suite** — `cargo test` (all non-ignored pass) and `cargo check` (clean).
- [ ] **Step 3: Frontend** — `npm test` (unchanged, pass).
- [ ] **Step 4: Privacy grep** — `rg -n "eCapture|microphone|mic|input device|recording device" src-tauri/src` → none.
- [ ] **Step 5: Commit** the gated test:
```powershell
git add src-tauri/tests/e2e_real_network.rs
git commit -m "test: gated siliconflow transcription roundtrip"
```

---

## Self-Review
- File ASR via finalize() seam, no trait/orchestration change: Tasks 1-2.
- WAV encoder pure + tested: Task 1.
- Buffer/finalize/error-survives-session/empty-transcript behavior: Task 2.
- reqwest multipart for the real transport: Task 2.
- ASR resolver from env, config struct, env only in commands: Task 3.
- Deterministic tests (mock transport); gated network smoke: Tasks 2,4.
- Key never leaked (errors use truncated body, not the key): Task 2.

Type consistency: `SiliconFlowFileConfig{base_url,api_key,model}`, `TranscriptionTransport::transcribe(&config,&[u8])->Result<String,AsrError>`, `SiliconFlowFileAsrClient::{connect,with_transport}`, `encode_wav_pcm16_mono`, `join_transcriptions_url`, `resolve_asr_client`/`resolve_asr_provider_name` consistent across tasks.

Out of scope: DashScope realtime ASR; long-utterance segment cap; provider settings UI.
