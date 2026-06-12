use respondent_lib::audio::frame::{AudioFrame, PcmFormat};

#[test]
fn computes_frame_duration_for_16khz_mono_pcm() {
    // 320 i16 samples at 16 kHz mono = 320 / 16000 s = 20 ms.
    let frame = AudioFrame {
        format: PcmFormat {
            sample_rate: 16_000,
            channels: 1,
            bits_per_sample: 16,
        },
        samples: vec![0; 320],
        captured_at_ms: 100,
    };

    assert_eq!(frame.duration_ms(), 20);
}
