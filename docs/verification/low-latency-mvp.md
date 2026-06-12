# Low-Latency MVP Verification

Date: 2026-06-12

## Automated Checks

- `npm test`
- `npm run build`
- `cd src-tauri && cargo test`
- `cd src-tauri && cargo check`

## Mock UI Flow

- Start creates a new visible listening session.
- Partial subtitle appears before final subtitle.
- Suggested reply starts only after endpoint/final sequence.
- Copy button copies the current suggestion.
- End changes state to Saved.

## Native App Flow

- `npm run tauri:dev` opens a Windows desktop window.
- Window stays above normal app windows.
- Top bar can drag the window.
- Start does not ask for microphone permission.
- Output device command returns at least one output device.

## Latency Checks

- Mock ASR partial target: under 800 ms.
- Mock endpoint target: 300 ms silence window.
- Mock reply first token target: under 1500 ms after endpoint.

## Privacy Checks

- No microphone API is requested in frontend code.
- No Rust microphone capture module exists.
- Session export contains text events only.
- Audio files are not written by default.
