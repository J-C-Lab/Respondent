use super::client::StreamingReplyClient;

pub struct MockReplyClient;

impl StreamingReplyClient for MockReplyClient {
    fn name(&self) -> &'static str {
        "mock-llm"
    }
}
