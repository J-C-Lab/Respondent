use crossbeam_channel::{bounded, Receiver, Sender};

use super::frame::{AudioFrame, PcmFormat};

pub struct LoopbackCapture {
    sender: Sender<AudioFrame>,
    receiver: Receiver<AudioFrame>,
}

impl LoopbackCapture {
    pub fn new_for_device(_device_id: &str) -> Self {
        let (sender, receiver) = bounded(128);
        Self { sender, receiver }
    }

    pub fn receiver(&self) -> Receiver<AudioFrame> {
        self.receiver.clone()
    }

    pub fn push_test_frame(&self, captured_at_ms: u64) {
        let _ = self.sender.send(AudioFrame {
            format: PcmFormat {
                sample_rate: 16_000,
                channels: 1,
                bits_per_sample: 16,
            },
            samples: vec![0; 320],
            captured_at_ms,
        });
    }
}
