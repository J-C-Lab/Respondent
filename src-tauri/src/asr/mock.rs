use super::client::StreamingAsrClient;

pub struct MockAsrClient;

impl StreamingAsrClient for MockAsrClient {
    fn name(&self) -> &'static str {
        "mock-asr"
    }
}
