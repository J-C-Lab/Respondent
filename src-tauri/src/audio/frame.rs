use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct PcmFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
}

#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub format: PcmFormat,
    pub samples: Vec<i16>,
    pub captured_at_ms: u64,
}

impl AudioFrame {
    pub fn duration_ms(&self) -> u32 {
        if self.format.sample_rate == 0 || self.format.channels == 0 {
            return 0;
        }
        let sample_frames = self.samples.len() as u32 / self.format.channels as u32;
        sample_frames * 1000 / self.format.sample_rate
    }
}
