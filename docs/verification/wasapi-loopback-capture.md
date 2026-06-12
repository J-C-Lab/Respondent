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
