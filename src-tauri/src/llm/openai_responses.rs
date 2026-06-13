use std::fmt;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use crossbeam_channel::{unbounded, Receiver};
use serde_json::{json, Value};

use super::client::{
    LlmError, ReplyEvent, ReplyGeneration, ReplyPoll, ReplyRequest, StreamingReplyClient,
};

const DEFAULT_OPENAI_REPLY_MODEL: &str = "gpt-5.4-mini";
const RESPONSES_URL: &str = "https://api.openai.com/v1/responses";
const GENERIC_FAILURE_TEXT: &str =
    "Reply generation failed. Check your OpenAI API key or network connection.";

#[derive(Clone, PartialEq, Eq)]
pub struct OpenAiReplyConfig {
    pub api_key: String,
    pub model: String,
}

impl fmt::Debug for OpenAiReplyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiReplyConfig")
            .field("api_key", &"<redacted>")
            .field("model", &self.model)
            .finish()
    }
}

impl OpenAiReplyConfig {
    pub fn from_api_key(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: std::env::var("OPENAI_LLM_MODEL")
                .ok()
                .filter(|model| !model.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_OPENAI_REPLY_MODEL.to_string()),
        }
    }
}

pub trait ResponsesTransport: Send + Sync {
    fn stream(
        &self,
        config: &OpenAiReplyConfig,
        request: &ReplyRequest,
    ) -> Result<Box<dyn ResponsesEventStream>, LlmError>;
}

pub trait ResponsesEventStream: Send {
    fn next_event(&mut self) -> Result<Option<Value>, LlmError>;
}

pub struct ReqwestResponsesTransport {
    client: reqwest::blocking::Client,
}

impl Default for ReqwestResponsesTransport {
    fn default() -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl ResponsesTransport for ReqwestResponsesTransport {
    fn stream(
        &self,
        config: &OpenAiReplyConfig,
        request: &ReplyRequest,
    ) -> Result<Box<dyn ResponsesEventStream>, LlmError> {
        let response = self
            .client
            .post(RESPONSES_URL)
            .bearer_auth(&config.api_key)
            .json(&build_responses_body(config, request))
            .send()
            .map_err(|err| LlmError::Provider(format!("openai responses request: {err}")))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            return Err(LlmError::Provider(format!(
                "openai responses http {status}: {}",
                truncate_for_error(&body)
            )));
        }

        Ok(Box::new(SseResponsesEventStream {
            reader: BufReader::new(response),
        }))
    }
}

struct SseResponsesEventStream {
    reader: BufReader<reqwest::blocking::Response>,
}

impl ResponsesEventStream for SseResponsesEventStream {
    fn next_event(&mut self) -> Result<Option<Value>, LlmError> {
        let mut line = String::new();
        loop {
            line.clear();
            let bytes = self
                .reader
                .read_line(&mut line)
                .map_err(|err| LlmError::Provider(format!("openai responses read: {err}")))?;
            if bytes == 0 {
                return Ok(None);
            }

            let trimmed = line.trim();
            let Some(data) = trimmed.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                return Ok(None);
            }

            let value = serde_json::from_str(data)
                .map_err(|err| LlmError::Provider(format!("openai responses json: {err}")))?;
            return Ok(Some(value));
        }
    }
}

pub struct OpenAiReplyClient {
    config: OpenAiReplyConfig,
    transport: Arc<dyn ResponsesTransport>,
}

impl OpenAiReplyClient {
    pub fn connect(config: OpenAiReplyConfig) -> Result<Self, LlmError> {
        Self::with_transport(config, Arc::new(ReqwestResponsesTransport::default()))
    }

    pub fn from_env() -> Result<Self, LlmError> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| LlmError::Provider("missing OPENAI_API_KEY".to_string()))?;
        Self::connect(OpenAiReplyConfig::from_api_key(api_key))
    }

    pub fn with_transport(
        config: OpenAiReplyConfig,
        transport: Arc<dyn ResponsesTransport>,
    ) -> Result<Self, LlmError> {
        if config.api_key.trim().is_empty() {
            return Err(LlmError::Provider("missing OPENAI_API_KEY".to_string()));
        }

        Ok(Self { config, transport })
    }
}

impl StreamingReplyClient for OpenAiReplyClient {
    fn name(&self) -> &'static str {
        "openai-responses-llm"
    }

    fn start(&self, request: ReplyRequest) -> Box<dyn ReplyGeneration> {
        Box::new(OpenAiReplyGeneration::start(
            self.config.clone(),
            Arc::clone(&self.transport),
            request,
        ))
    }
}

pub fn build_responses_body(config: &OpenAiReplyConfig, request: &ReplyRequest) -> Value {
    json!({
        "model": config.model,
        "stream": true,
        "input": [
            {
                "role": "system",
                "content": "You are a live meeting assistant. Suggest one concise, useful reply the user could say next. Keep it natural, specific, and short."
            },
            {
                "role": "user",
                "content": format!(
                    "Conversation context:\n{}\n\nCurrent turn:\n{}\n\nWrite the suggested reply only.",
                    format_context(&request.context),
                    request.transcript
                )
            }
        ]
    })
}

struct OpenAiReplyGeneration {
    receiver: Receiver<ReplyPoll>,
    done: bool,
}

impl OpenAiReplyGeneration {
    fn start(
        config: OpenAiReplyConfig,
        transport: Arc<dyn ResponsesTransport>,
        request: ReplyRequest,
    ) -> Self {
        let (sender, receiver) = unbounded();
        let _ = sender.send(ReplyPoll::Event(ReplyEvent::Started {
            session_id: request.session_id.clone(),
            generation_id: request.generation_id.clone(),
            based_on_transcript_event_id: format!("transcript-{}", request.generation_id),
            received_at_ms: now_ms(),
        }));

        thread::Builder::new()
            .name("openai-responses-llm".into())
            .spawn(move || {
                let session_id = request.session_id.clone();
                let generation_id = request.generation_id.clone();
                let mut final_text = String::new();
                let stream = transport.stream(&config, &request);
                let mut stream = match stream {
                    Ok(stream) => stream,
                    Err(_) => {
                        send_failure_final(&sender, &session_id, &generation_id);
                        let _ = sender.send(ReplyPoll::Done);
                        return;
                    }
                };

                loop {
                    match stream.next_event() {
                        Ok(Some(event)) => match event["type"].as_str() {
                            Some("response.output_text.delta") => {
                                if let Some(delta) = event["delta"].as_str() {
                                    final_text.push_str(delta);
                                    let _ = sender.send(ReplyPoll::Event(ReplyEvent::Token {
                                        session_id: session_id.clone(),
                                        generation_id: generation_id.clone(),
                                        token: delta.to_string(),
                                        received_at_ms: now_ms(),
                                    }));
                                }
                            }
                            Some("response.completed") => {
                                send_final(&sender, &session_id, &generation_id, final_text);
                                let _ = sender.send(ReplyPoll::Done);
                                return;
                            }
                            Some("response.error") | Some("error") => {
                                send_failure_final(&sender, &session_id, &generation_id);
                                let _ = sender.send(ReplyPoll::Done);
                                return;
                            }
                            _ => {}
                        },
                        Ok(None) => {
                            if final_text.is_empty() {
                                send_failure_final(&sender, &session_id, &generation_id);
                            } else {
                                send_final(&sender, &session_id, &generation_id, final_text);
                            }
                            let _ = sender.send(ReplyPoll::Done);
                            return;
                        }
                        Err(_) => {
                            send_failure_final(&sender, &session_id, &generation_id);
                            let _ = sender.send(ReplyPoll::Done);
                            return;
                        }
                    }
                }
            })
            .expect("spawn openai responses llm worker");

        Self {
            receiver,
            done: false,
        }
    }
}

impl ReplyGeneration for OpenAiReplyGeneration {
    fn poll(&mut self) -> ReplyPoll {
        if self.done {
            return ReplyPoll::Done;
        }

        match self.receiver.try_recv() {
            Ok(ReplyPoll::Done) => {
                self.done = true;
                ReplyPoll::Done
            }
            Ok(poll) => poll,
            Err(crossbeam_channel::TryRecvError::Empty) => ReplyPoll::Pending,
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                self.done = true;
                ReplyPoll::Done
            }
        }
    }
}

fn send_final(
    sender: &crossbeam_channel::Sender<ReplyPoll>,
    session_id: &str,
    generation_id: &str,
    text: String,
) {
    let _ = sender.send(ReplyPoll::Event(ReplyEvent::Final {
        session_id: session_id.to_string(),
        generation_id: generation_id.to_string(),
        text,
        received_at_ms: now_ms(),
    }));
}

fn send_failure_final(
    sender: &crossbeam_channel::Sender<ReplyPoll>,
    session_id: &str,
    generation_id: &str,
) {
    send_final(
        sender,
        session_id,
        generation_id,
        GENERIC_FAILURE_TEXT.to_string(),
    );
}

fn format_context(context: &[String]) -> String {
    if context.is_empty() {
        return "(none)".to_string();
    }

    context
        .iter()
        .map(|turn| format!("- {turn}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate_for_error(text: &str) -> String {
    const LIMIT: usize = 240;
    let trimmed = text.trim();
    if trimmed.len() <= LIMIT {
        return trimmed.to_string();
    }
    format!("{}...", &trimmed[..LIMIT])
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
