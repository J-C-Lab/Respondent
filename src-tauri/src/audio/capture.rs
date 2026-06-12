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
        Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
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
