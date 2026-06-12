# WASAPI Loopback Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the mock `LoopbackCapture` skeleton with a Windows 10+ WASAPI loopback capture pipeline that emits 16 kHz mono i16 `AudioFrame`s through the existing `receiver()` seam.

**Architecture:** Keep audio conversion pure and fully unit-tested in `convert.rs`, then keep COM/WASAPI code isolated in `capture.rs` behind a small lifecycle API. The capture thread must never block on downstream consumers; on channel backpressure it drops the newest frame and increments a dropped-frame counter.

**Tech Stack:** Rust, Tauri backend crate, Windows WASAPI via the `windows` crate, `crossbeam-channel`, `thiserror`, Rust unit/integration tests.

---

## Scope Check

This plan implements only the first real-pipeline subsystem: Windows system-output audio capture. It does not connect streaming ASR, streaming LLM, device-switch reconnects, friendly device names, UI settings, or microphone/input capture.

The work is split into independently reviewable tasks:

- pure conversion constants and helpers
- streaming resampler and frame chunker
- capture pipeline composition
- capture lifecycle and backpressure contract
- Windows format parsing and WASAPI loopback implementation
- ignored manual smoke test and verification docs

## File Structure

Files to create or modify:

```text
src-tauri/
  Cargo.toml
  src/audio/
    mod.rs
    frame.rs
    convert.rs
    capture.rs
  tests/
    audio_contract.rs
    loopback_capture_smoke.rs
docs/
  verification/
    wasapi-loopback-capture.md
```

Responsibilities:

- `convert.rs`: pure conversion only; no `windows`, no COM, no threads.
- `capture.rs`: lifecycle, thread, channel backpressure, platform-specific WASAPI.
- `audio_contract.rs`: non-hardware tests for pure logic and public contracts.
- `loopback_capture_smoke.rs`: ignored hardware/manual test that requires playing system audio.
- `wasapi-loopback-capture.md`: manual verification checklist for real audio.

## Task 1: Conversion Constants, Downmix, Quantization

**Files:**
- Create: `src-tauri/src/audio/convert.rs`
- Modify: `src-tauri/src/audio/mod.rs`
- Modify: `src-tauri/tests/audio_contract.rs`

- [ ] **Step 1: Write failing tests for constants, downmix, and quantization**

Append to `src-tauri/tests/audio_contract.rs`:

```rust
use respondent_lib::audio::convert::{
    downmix_to_mono, to_pcm16, TARGET_CHANNELS, TARGET_FRAME_SAMPLES, TARGET_RATE,
};

#[test]
fn target_capture_format_is_16khz_mono_20ms() {
    assert_eq!(TARGET_RATE, 16_000);
    assert_eq!(TARGET_CHANNELS, 1);
    assert_eq!(TARGET_FRAME_SAMPLES, 320);
}

#[test]
fn downmixes_stereo_to_mono_by_averaging_channels() {
    let mono = downmix_to_mono(&[1.0, -1.0, 0.5, 0.25], 2);
    assert_eq!(mono, vec![0.0, 0.375]);
}

#[test]
fn downmix_mono_returns_input_samples() {
    let mono = downmix_to_mono(&[0.25, -0.5], 1);
    assert_eq!(mono, vec![0.25, -0.5]);
}

#[test]
fn downmix_zero_channels_returns_empty_output() {
    assert!(downmix_to_mono(&[1.0, 2.0], 0).is_empty());
}

#[test]
fn quantizes_float_samples_to_pcm16_with_clamp() {
    assert_eq!(to_pcm16(&[1.5, -1.5, 0.0, 0.5]), vec![32767, -32767, 0, 16384]);
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
cargo test --test audio_contract
```

Expected: fail with unresolved import `respondent_lib::audio::convert`.

- [ ] **Step 3: Implement conversion constants and helpers**

Create `src-tauri/src/audio/convert.rs`:

```rust
pub const TARGET_RATE: u32 = 16_000;
pub const TARGET_CHANNELS: u16 = 1;
pub const TARGET_BITS_PER_SAMPLE: u16 = 16;
pub const TARGET_FRAME_SAMPLES: usize = 320;

pub fn downmix_to_mono(interleaved: &[f32], channels: u16) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }
    if channels == 1 {
        return interleaved.to_vec();
    }

    let channels = channels as usize;
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

pub fn to_pcm16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|sample| {
            let clamped = sample.clamp(-1.0, 1.0);
            (clamped * i16::MAX as f32).round() as i16
        })
        .collect()
}
```

Modify `src-tauri/src/audio/mod.rs`:

```rust
pub mod capture;
pub mod convert;
pub mod devices;
pub mod frame;
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
cargo test --test audio_contract
```

Expected: all `audio_contract` tests pass.

- [ ] **Step 5: Commit**

Run:

```powershell
git add src-tauri/src/audio/convert.rs src-tauri/src/audio/mod.rs src-tauri/tests/audio_contract.rs
git commit -m "feat: add audio conversion primitives"
```

## Task 2: Streaming Resampler And Frame Chunker

**Files:**
- Modify: `src-tauri/src/audio/convert.rs`
- Modify: `src-tauri/tests/audio_contract.rs`

- [ ] **Step 1: Write failing tests for resampling and chunking**

Append to `src-tauri/tests/audio_contract.rs`:

```rust
use respondent_lib::audio::convert::{FrameChunker, LinearResampler};

#[test]
fn resampler_passes_through_when_rates_match() {
    let mut resampler = LinearResampler::new(16_000, 16_000);
    assert_eq!(resampler.process(&[0.0, 0.5, 1.0]), vec![0.0, 0.5, 1.0]);
}

#[test]
fn resampler_downsamples_48khz_to_16khz_at_integer_ratio() {
    let mut resampler = LinearResampler::new(48_000, 16_000);
    assert_eq!(resampler.process(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), vec![0.0, 3.0]);
}

#[test]
fn resampler_keeps_fractional_ratio_output_monotonic() {
    let mut resampler = LinearResampler::new(44_100, 16_000);
    let output = resampler.process(&(0..100).map(|value| value as f32).collect::<Vec<_>>());
    assert!((35..=37).contains(&output.len()));
    assert!(output.windows(2).all(|pair| pair[0] <= pair[1]));
}

#[test]
fn resampler_is_continuous_across_chunks() {
    let input = (0..128).map(|value| value as f32).collect::<Vec<_>>();
    let mut one_pass = LinearResampler::new(48_000, 16_000);
    let expected = one_pass.process(&input);

    let mut chunked = LinearResampler::new(48_000, 16_000);
    let mut actual = chunked.process(&input[..64]);
    actual.extend(chunked.process(&input[64..]));

    assert_eq!(actual, expected);
}

#[test]
fn frame_chunker_emits_full_320_sample_frames_and_retains_remainder() {
    let mut chunker = FrameChunker::new();
    let first = chunker.push(&vec![1; 800]);
    assert_eq!(first.len(), 2);
    assert!(first.iter().all(|frame| frame.len() == 320));
    assert_eq!(chunker.pending_len(), 160);

    let second = chunker.push(&vec![2; 160]);
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].len(), 320);
    assert_eq!(chunker.pending_len(), 0);
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
cargo test --test audio_contract
```

Expected: fail with missing `LinearResampler` and `FrameChunker`.

- [ ] **Step 3: Implement resampler and chunker**

Add to `src-tauri/src/audio/convert.rs`:

```rust
#[derive(Debug, Clone)]
pub struct LinearResampler {
    src_rate: u32,
    dst_rate: u32,
    pos: f64,
    last: Option<f32>,
}

impl LinearResampler {
    pub fn new(src_rate: u32, dst_rate: u32) -> Self {
        Self {
            src_rate,
            dst_rate,
            pos: 0.0,
            last: None,
        }
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if input.is_empty() {
            return Vec::new();
        }
        if self.src_rate == self.dst_rate {
            self.last = input.last().copied();
            return input.to_vec();
        }
        if self.src_rate == 0 || self.dst_rate == 0 {
            self.last = input.last().copied();
            return Vec::new();
        }

        let mut extended = Vec::with_capacity(input.len() + usize::from(self.last.is_some()));
        if let Some(last) = self.last {
            extended.push(last);
        }
        extended.extend_from_slice(input);

        let offset = if self.last.is_some() { 1.0 } else { 0.0 };
        let step = self.src_rate as f64 / self.dst_rate as f64;
        let mut output = Vec::new();

        while self.pos + 1.0 < extended.len() as f64 {
            let left_index = self.pos.floor() as usize;
            let frac = (self.pos - left_index as f64) as f32;
            let left = extended[left_index];
            let right = extended[left_index + 1];
            output.push(left + (right - left) * frac);
            self.pos += step;
        }

        self.pos -= input.len() as f64;
        if self.last.is_some() {
            self.pos += offset;
        }
        if self.pos < 0.0 {
            self.pos = 0.0;
        }
        self.last = input.last().copied();

        output
    }
}

#[derive(Debug, Default, Clone)]
pub struct FrameChunker {
    buf: Vec<i16>,
}

impl FrameChunker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, samples: &[i16]) -> Vec<Vec<i16>> {
        self.buf.extend_from_slice(samples);
        let mut frames = Vec::new();
        while self.buf.len() >= TARGET_FRAME_SAMPLES {
            frames.push(self.buf.drain(..TARGET_FRAME_SAMPLES).collect());
        }
        frames
    }

    pub fn pending_len(&self) -> usize {
        self.buf.len()
    }
}
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
cargo test --test audio_contract
```

Expected: all `audio_contract` tests pass. If the continuity test fails, fix the position normalization; do not loosen the test unless the expected one-pass comparison is mathematically wrong.

- [ ] **Step 5: Commit**

Run:

```powershell
git add src-tauri/src/audio/convert.rs src-tauri/tests/audio_contract.rs
git commit -m "feat: add streaming audio resampler"
```

## Task 3: Capture Pipeline Composition

**Files:**
- Modify: `src-tauri/src/audio/convert.rs`
- Modify: `src-tauri/tests/audio_contract.rs`

- [ ] **Step 1: Write failing pipeline test**

Append to `src-tauri/tests/audio_contract.rs`:

```rust
use respondent_lib::audio::convert::CapturePipeline;

#[test]
fn capture_pipeline_outputs_16khz_mono_pcm_frames() {
    let mut pipeline = CapturePipeline::new(48_000, 2);
    let stereo = (0..1_920)
        .flat_map(|index| {
            let sample = ((index % 48) as f32) / 48.0;
            [sample, sample]
        })
        .collect::<Vec<_>>();

    let frames = pipeline.push_interleaved_f32(&stereo, 42);

    assert!(!frames.is_empty());
    assert!(frames.iter().all(|frame| frame.format.sample_rate == 16_000));
    assert!(frames.iter().all(|frame| frame.format.channels == 1));
    assert!(frames.iter().all(|frame| frame.format.bits_per_sample == 16));
    assert!(frames.iter().all(|frame| frame.samples.len() == 320));
    assert!(frames.iter().all(|frame| frame.captured_at_ms == 42));
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
cargo test --test audio_contract
```

Expected: fail with missing `CapturePipeline`.

- [ ] **Step 3: Implement `CapturePipeline`**

Add to `src-tauri/src/audio/convert.rs`:

```rust
use super::frame::{AudioFrame, PcmFormat};

#[derive(Debug, Clone)]
pub struct CapturePipeline {
    resampler: LinearResampler,
    chunker: FrameChunker,
    channels: u16,
}

impl CapturePipeline {
    pub fn new(src_rate: u32, channels: u16) -> Self {
        Self {
            resampler: LinearResampler::new(src_rate, TARGET_RATE),
            chunker: FrameChunker::new(),
            channels,
        }
    }

    pub fn push_interleaved_f32(
        &mut self,
        interleaved: &[f32],
        captured_at_ms: u64,
    ) -> Vec<AudioFrame> {
        let mono = downmix_to_mono(interleaved, self.channels);
        let resampled = self.resampler.process(&mono);
        let pcm = to_pcm16(&resampled);
        self.chunker
            .push(&pcm)
            .into_iter()
            .map(|samples| AudioFrame {
                format: PcmFormat {
                    sample_rate: TARGET_RATE,
                    channels: TARGET_CHANNELS,
                    bits_per_sample: TARGET_BITS_PER_SAMPLE,
                },
                samples,
                captured_at_ms,
            })
            .collect()
    }
}
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
cargo test --test audio_contract
```

Expected: all `audio_contract` tests pass.

- [ ] **Step 5: Commit**

Run:

```powershell
git add src-tauri/src/audio/convert.rs src-tauri/tests/audio_contract.rs
git commit -m "feat: compose capture conversion pipeline"
```

## Task 4: Capture Lifecycle Contract And Backpressure

**Files:**
- Modify: `src-tauri/src/audio/capture.rs`
- Modify: `src-tauri/tests/audio_contract.rs`

- [ ] **Step 1: Write failing lifecycle/backpressure tests**

Append to `src-tauri/tests/audio_contract.rs`:

```rust
use respondent_lib::audio::capture::LoopbackCapture;

#[test]
fn loopback_capture_test_constructor_preserves_receiver_contract() {
    let capture = LoopbackCapture::new_for_device("default-output");
    let receiver = capture.receiver();

    capture.push_test_frame(123);
    let frame = receiver.try_recv().expect("test frame");

    assert_eq!(frame.captured_at_ms, 123);
    assert_eq!(frame.format.sample_rate, 16_000);
    assert_eq!(frame.samples.len(), 320);
}

#[test]
fn loopback_capture_drops_new_frames_when_test_channel_is_full() {
    let capture = LoopbackCapture::new_for_test_with_capacity(1);
    capture.push_test_frame(1);
    capture.push_test_frame(2);

    assert_eq!(capture.dropped_frames(), 1);
    let frame = capture.receiver().try_recv().expect("old frame is retained");
    assert_eq!(frame.captured_at_ms, 1);
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
cargo test --test audio_contract
```

Expected: fail with missing `new_for_test_with_capacity` or `dropped_frames`.

- [ ] **Step 3: Implement lifecycle-compatible test constructor and backpressure**

Replace `src-tauri/src/audio/capture.rs` with:

```rust
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use thiserror::Error;

use super::frame::{AudioFrame, PcmFormat};

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("unsupported capture configuration: {0}")]
    Unsupported(String),
    #[error("capture thread join failed: {0}")]
    ThreadJoin(String),
    #[cfg(target_os = "windows")]
    #[error("windows audio error: {0}")]
    Com(#[from] windows::core::Error),
}

pub struct LoopbackCapture {
    sender: Sender<AudioFrame>,
    receiver: Receiver<AudioFrame>,
    dropped_frames: Arc<AtomicU64>,
}

impl LoopbackCapture {
    pub fn new_for_device(_device_id: &str) -> Self {
        Self::new_for_test_with_capacity(128)
    }

    pub fn new_for_test_with_capacity(capacity: usize) -> Self {
        let (sender, receiver) = bounded(capacity);
        Self {
            sender,
            receiver,
            dropped_frames: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn start(device_id: &str) -> Result<Self, CaptureError> {
        start_platform_capture(device_id)
    }

    pub fn receiver(&self) -> Receiver<AudioFrame> {
        self.receiver.clone()
    }

    pub fn dropped_frames(&self) -> u64 {
        self.dropped_frames.load(Ordering::Relaxed)
    }

    pub fn stop(self) -> Result<(), CaptureError> {
        Ok(())
    }

    pub fn push_test_frame(&self, captured_at_ms: u64) {
        let frame = AudioFrame {
            format: PcmFormat {
                sample_rate: 16_000,
                channels: 1,
                bits_per_sample: 16,
            },
            samples: vec![0; 320],
            captured_at_ms,
        };
        send_or_drop_newest(&self.sender, &self.dropped_frames, frame);
    }
}

fn send_or_drop_newest(
    sender: &Sender<AudioFrame>,
    dropped_frames: &Arc<AtomicU64>,
    frame: AudioFrame,
) {
    match sender.try_send(frame) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            dropped_frames.fetch_add(1, Ordering::Relaxed);
        }
        Err(TrySendError::Disconnected(_)) => {
            dropped_frames.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn start_platform_capture(_device_id: &str) -> Result<LoopbackCapture, CaptureError> {
    Err(CaptureError::Unsupported(
        "event-driven loopback requires Windows 10 or later".into(),
    ))
}

#[cfg(target_os = "windows")]
fn start_platform_capture(_device_id: &str) -> Result<LoopbackCapture, CaptureError> {
    Err(CaptureError::Unsupported(
        "real WASAPI loopback capture is implemented in a later task".into(),
    ))
}
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
cargo test --test audio_contract
cargo check
```

Expected: tests and check pass.

- [ ] **Step 5: Commit**

Run:

```powershell
git add src-tauri/src/audio/capture.rs src-tauri/tests/audio_contract.rs
git commit -m "feat: define loopback capture lifecycle"
```

## Task 5: WASAPI Format Parsing

**Files:**
- Modify: `src-tauri/src/audio/capture.rs`
- Modify: `src-tauri/tests/audio_contract.rs`

- [ ] **Step 1: Write failing tests for supported and unsupported sample formats**

Append to `src-tauri/tests/audio_contract.rs`:

```rust
use respondent_lib::audio::capture::{SampleFormat, WasapiFormat};

#[test]
fn accepts_float32_and_pcm16_formats() {
    assert_eq!(
        WasapiFormat::new(48_000, 2, 32, SampleFormat::Float32).expect("float32").sample_format,
        SampleFormat::Float32
    );
    assert_eq!(
        WasapiFormat::new(48_000, 2, 16, SampleFormat::Pcm16).expect("pcm16").sample_format,
        SampleFormat::Pcm16
    );
}

#[test]
fn rejects_unsupported_pcm_bit_depths() {
    let err = WasapiFormat::new(48_000, 2, 24, SampleFormat::Pcm16).expect_err("pcm24 rejected");
    assert!(err.to_string().contains("unsupported"));
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
cargo test --test audio_contract
```

Expected: fail with missing `SampleFormat` and `WasapiFormat`.

- [ ] **Step 3: Implement public format model**

Add to `src-tauri/src/audio/capture.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    Float32,
    Pcm16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasapiFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub sample_format: SampleFormat,
}

impl WasapiFormat {
    pub fn new(
        sample_rate: u32,
        channels: u16,
        bits_per_sample: u16,
        sample_format: SampleFormat,
    ) -> Result<Self, CaptureError> {
        match (sample_format, bits_per_sample) {
            (SampleFormat::Float32, 32) | (SampleFormat::Pcm16, 16) => Ok(Self {
                sample_rate,
                channels,
                bits_per_sample,
                sample_format,
            }),
            _ => Err(CaptureError::Unsupported(format!(
                "unsupported WASAPI format: {:?} {}-bit",
                sample_format, bits_per_sample
            ))),
        }
    }
}
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
cargo test --test audio_contract
cargo check
```

Expected: tests and check pass.

- [ ] **Step 5: Commit**

Run:

```powershell
git add src-tauri/src/audio/capture.rs src-tauri/tests/audio_contract.rs
git commit -m "feat: add WASAPI format contract"
```

## Task 6: Windows WASAPI Loopback Thread

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/audio/capture.rs`

- [ ] **Step 1: Add Windows feature dependencies if missing**

Ensure `src-tauri/Cargo.toml` includes these Windows features:

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_Media_Audio",
  "Win32_Media_KernelStreaming",
  "Win32_System_Com",
  "Win32_System_Threading",
  "Win32_UI_Shell_PropertiesSystem",
  "Win32_Devices_FunctionDiscovery",
] }
```

- [ ] **Step 2: Implement Windows `start_platform_capture`**

Replace the Windows stub in `src-tauri/src/audio/capture.rs` with a Windows-only implementation that:

- Creates a bounded channel with capacity 128.
- Creates a stop flag shared with the thread.
- Starts a named capture thread.
- Initializes COM with `COINIT_MULTITHREADED`.
- Uses render endpoint loopback only.
- Calls `IAudioClient::Initialize` in shared mode with `AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK`.
- Calls `SetEventHandle` before `Start`.
- Reads `IAudioCaptureClient` packets until stop.
- Converts float32/PCM16 interleaved samples into f32.
- Pushes through `CapturePipeline`.
- Uses `send_or_drop_newest`.
- Stops the audio client before thread exit.

The implementation may add private Windows helpers, but it must not introduce microphone/eCapture paths.

- [ ] **Step 3: Run build checks**

Run:

```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
cargo check
cargo test --test audio_contract
```

Expected: `cargo check` compiles Windows-specific code and `audio_contract` passes.

- [ ] **Step 4: Commit**

Run:

```powershell
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/audio/capture.rs
git commit -m "feat: implement WASAPI loopback capture"
```

## Task 7: Ignored Smoke Test And Verification Checklist

**Files:**
- Create: `src-tauri/tests/loopback_capture_smoke.rs`
- Create: `docs/verification/wasapi-loopback-capture.md`

- [ ] **Step 1: Add ignored smoke test**

Create `src-tauri/tests/loopback_capture_smoke.rs`:

```rust
use std::time::{Duration, Instant};

use respondent_lib::audio::capture::LoopbackCapture;

#[test]
#[ignore = "requires audible system output on Windows"]
fn loopback_capture_receives_non_silent_16khz_frames() {
    let capture = LoopbackCapture::start("default-output").expect("start loopback capture");
    let receiver = capture.receiver();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut saw_non_silent = false;

    while Instant::now() < deadline {
        if let Ok(frame) = receiver.recv_timeout(Duration::from_millis(250)) {
            assert_eq!(frame.format.sample_rate, 16_000);
            assert_eq!(frame.format.channels, 1);
            assert_eq!(frame.format.bits_per_sample, 16);
            assert_eq!(frame.samples.len(), 320);
            saw_non_silent |= frame.samples.iter().any(|sample| *sample != 0);
            if saw_non_silent {
                break;
            }
        }
    }

    capture.stop().expect("stop capture");
    assert!(saw_non_silent, "play system audio while running this ignored test");
}
```

- [ ] **Step 2: Add manual verification checklist**

Create `docs/verification/wasapi-loopback-capture.md`:

```markdown
# WASAPI Loopback Capture Verification

Date: 2026-06-12

## Automated

- `cargo test --test audio_contract`
- `cargo check`
- `cargo test -- --ignored` while system audio is playing

## Manual

- Start playback through the default Windows output device.
- Start `LoopbackCapture::start("default-output")`.
- Confirm at least one `AudioFrame` arrives within 5 seconds.
- Confirm frames are 16 kHz, mono, i16, 320 samples.
- Confirm at least one frame has a non-zero sample.
- Stop capture and confirm `stop()` returns `Ok`.
- Confirm no microphone permission prompt appears.

## Privacy Contract

- Code uses render endpoint loopback only.
- No `eCapture` input endpoint path exists.
- No raw audio file is written by this stage.
```

- [ ] **Step 3: Run normal tests**

Run:

```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
cargo test
cargo check
```

Expected: normal tests pass; ignored smoke test is not run by default.

- [ ] **Step 4: Run gated smoke test manually**

Start audible system playback, then run:

```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
cargo test -- --ignored
```

Expected: smoke test passes when audio is playing. If no audio is playing, it may fail with the explicit message.

- [ ] **Step 5: Commit**

Run:

```powershell
git add src-tauri/tests/loopback_capture_smoke.rs docs/verification/wasapi-loopback-capture.md
git commit -m "test: add WASAPI loopback smoke verification"
```

## Task 8: Full Verification

**Files:**
- No new files expected.

- [ ] **Step 1: Run frontend tests**

Run:

```powershell
npm test
npm run build
```

Expected: all frontend tests pass and Vite build completes.

- [ ] **Step 2: Run Rust tests**

Run:

```powershell
$env:Path = 'C:\Users\Administrator\.cargo\bin;' + $env:Path
cargo test
cargo check
```

Expected: all non-ignored Rust tests pass and crate checks.

- [ ] **Step 3: Confirm privacy grep**

Run:

```powershell
rg -n "eCapture|microphone|mic|input device|recording device" src-tauri/src src
```

Expected: no implementation path that opens microphone/input capture. Documentation or negative tests may mention microphone as a prohibited path.

- [ ] **Step 4: Commit any verification-only doc adjustments**

If no files changed, do not create an empty commit. If verification docs were updated, run:

```powershell
git add docs/verification
git commit -m "docs: update WASAPI verification notes"
```

## Self-Review

Spec coverage:

- Windows 10+ event-driven loopback: Task 6.
- No microphone/input capture: Task 6 and Task 8 grep.
- 16 kHz mono i16 frames: Tasks 1-3 and Task 7.
- Backpressure drop-newest policy: Task 4.
- Supported format contract: Task 5 and Task 6.
- Ignored smoke test and manual verification: Task 7.
- Existing `receiver()` seam: Tasks 4 and 7.

Red-flag scan:

- No empty implementation markers or undefined task bodies remain.

Type consistency:

- `AudioFrame`, `PcmFormat`, and `LoopbackCapture::receiver()` match the existing audio module.
- `CaptureError` is defined in `capture.rs` and used by lifecycle APIs.
- `CapturePipeline` outputs existing `AudioFrame` values.
