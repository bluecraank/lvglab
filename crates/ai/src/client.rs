//! Minimal client for the Claude Messages API (raw HTTPS; there is no
//! official Rust SDK). Streams the response so long designs do not hit
//! HTTP timeouts.

use std::io::{BufRead, BufReader};
use std::time::Duration;

use serde_json::{Value, json};

use crate::spec::{Design, schema};

pub const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
/// Server-side fallback when a safety classifier declines (`fallbacks: "default"`).
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
const MAX_TOKENS: u32 = 64000;

/// Models offered in the editor, most capable first.
pub const MODELS: &[(&str, &str)] = &[
    ("claude-opus-5-5", "Claude Opus 5.5"),
    ("claude-sonnet-5-5", "Claude Sonnet 5.5"),
    ("claude-haiku-5-5", "Claude Haiku 5.5"),
];

/// Effort levels (`output_config.effort`).
pub const EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

#[derive(Clone, Debug)]
pub struct Config {
    pub api_key: String,
    pub model: String,
    pub effort: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("no API key configured")]
    MissingKey,
    #[error("the API key was rejected")]
    Unauthorized,
    #[error("rate limit reached, try again in a moment")]
    RateLimited,
    #[error("the service is overloaded, try again in a moment")]
    Overloaded,
    #[error("API error {status}: {message}")]
    Api { status: u16, message: String },
    #[error("network error: {0}")]
    Network(String),
    #[error("the request was declined by the safety system")]
    Refused,
    #[error("the answer was cut off (output limit reached)")]
    Truncated,
    #[error("the answer could not be read: {0}")]
    InvalidAnswer(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
}

#[derive(Debug)]
pub struct Answer {
    pub design: Design,
    pub usage: Usage,
    /// The model that produced the answer (may be a fallback model).
    pub model: String,
}

/// Builds the request body. Kept separate for tests.
pub fn request_body(config: &Config, system: &str, user: &str) -> Value {
    let mut body = json!({
        "model": config.model,
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "system": [{ "type": "text", "text": system, "cache_control": { "type": "ephemeral" } }],
        "messages": [{ "role": "user", "content": user }],
        "output_config": {
            "effort": config.effort,
            "format": { "type": "json_schema", "schema": schema() }
        }
    });
    if uses_fallbacks(&config.model) {
        body["fallbacks"] = json!("default");
    }
    body
}

fn uses_fallbacks(model: &str) -> bool {
    matches!(model, "claude-opus-5-5" | "claude-sonnet-5-5")
}

/// Sends the request and returns the parsed design. `on_progress` receives
/// the number of characters received so far.
pub fn generate(
    config: &Config,
    system: &str,
    user: &str,
    mut on_progress: impl FnMut(usize),
) -> Result<Answer, AiError> {
    if config.api_key.trim().is_empty() {
        return Err(AiError::MissingKey);
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(300))
        .build();
    let mut request = agent
        .post(API_URL)
        .set("content-type", "application/json")
        .set("x-api-key", config.api_key.trim())
        .set("anthropic-version", API_VERSION);
    if uses_fallbacks(&config.model) {
        request = request.set("anthropic-beta", FALLBACK_BETA);
    }
    let response = match request.send_json(request_body(config, system, user)) {
        Ok(response) => response,
        Err(ureq::Error::Status(status, response)) => return Err(status_error(status, response)),
        Err(err) => return Err(AiError::Network(err.to_string())),
    };
    let stream = read_stream(BufReader::new(response.into_reader()), &mut on_progress)?;
    match stream.stop_reason.as_deref() {
        Some("refusal") => return Err(AiError::Refused),
        Some("max_tokens") => return Err(AiError::Truncated),
        _ => {}
    }
    let design =
        serde_json::from_str::<Design>(stream.text.trim()).map_err(|e| AiError::InvalidAnswer(e.to_string()))?;
    Ok(Answer { design, usage: stream.usage, model: stream.model })
}

fn status_error(status: u16, response: ureq::Response) -> AiError {
    let message = response
        .into_json::<Value>()
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
        .unwrap_or_default();
    match status {
        401 | 403 => AiError::Unauthorized,
        429 => AiError::RateLimited,
        529 => AiError::Overloaded,
        _ => AiError::Api { status, message },
    }
}

#[derive(Debug, Default)]
struct Stream {
    text: String,
    stop_reason: Option<String>,
    usage: Usage,
    model: String,
}

/// Reads the server-sent events of a streamed message.
fn read_stream(reader: impl BufRead, on_progress: &mut impl FnMut(usize)) -> Result<Stream, AiError> {
    let mut stream = Stream::default();
    for line in reader.lines() {
        let line = line.map_err(|e| AiError::Network(e.to_string()))?;
        let Some(data) = line.strip_prefix("data:") else { continue };
        let Ok(event) = serde_json::from_str::<Value>(data.trim()) else { continue };
        match event["type"].as_str().unwrap_or_default() {
            "message_start" => {
                let message = &event["message"];
                stream.model = message["model"].as_str().unwrap_or_default().to_owned();
                stream.usage.input_tokens = message["usage"]["input_tokens"].as_u64().unwrap_or(0);
                stream.usage.cache_read_tokens = message["usage"]["cache_read_input_tokens"].as_u64().unwrap_or(0);
            }
            "content_block_delta" => {
                if event["delta"]["type"] == "text_delta" {
                    stream.text.push_str(event["delta"]["text"].as_str().unwrap_or_default());
                    on_progress(stream.text.len());
                }
            }
            "message_delta" => {
                if let Some(reason) = event["delta"]["stop_reason"].as_str() {
                    stream.stop_reason = Some(reason.to_owned());
                }
                if let Some(output) = event["usage"]["output_tokens"].as_u64() {
                    stream.usage.output_tokens = output;
                }
            }
            "error" => {
                let message = event["error"]["message"].as_str().unwrap_or_default().to_owned();
                return Err(match event["error"]["type"].as_str() {
                    Some("overloaded_error") => AiError::Overloaded,
                    Some("rate_limit_error") => AiError::RateLimited,
                    _ => AiError::Api { status: 200, message },
                });
            }
            _ => {}
        }
    }
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(model: &str) -> Config {
        Config { api_key: "k".into(), model: model.into(), effort: "medium".into() }
    }

    #[test]
    fn body_uses_structured_output_caching_and_fallbacks() {
        let body = request_body(&config("claude-opus-5-5"), "sys", "user");
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert_eq!(body["output_config"]["effort"], "medium");
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
        assert_eq!(body["fallbacks"], "default");
        assert!(body.get("thinking").is_none(), "Opus 5.5 always thinks adaptively");
        let haiku = request_body(&config("claude-haiku-5-5"), "sys", "user");
        assert!(haiku.get("fallbacks").is_none());
    }

    #[test]
    fn parses_event_stream() {
        let events = "event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-opus-5-5\",\"usage\":{\"input_tokens\":10,\"cache_read_input_tokens\":8}}}\n\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"{\\\"summary\\\":\"}}\n\n\
data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"\\\"ok\\\",\\\"screens\\\":[]}\"}}\n\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\n";
        let mut progress = 0;
        let stream = read_stream(events.as_bytes(), &mut |n| progress = n).unwrap();
        assert_eq!(stream.text, r#"{"summary":"ok","screens":[]}"#);
        assert_eq!(stream.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(stream.usage, Usage { input_tokens: 10, output_tokens: 5, cache_read_tokens: 8 });
        assert_eq!(progress, stream.text.len());
    }

    #[test]
    fn stream_errors_are_reported() {
        let events = "data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"busy\"}}\n";
        assert!(matches!(read_stream(events.as_bytes(), &mut |_| {}), Err(AiError::Overloaded)));
    }

    #[test]
    fn missing_key_fails_early() {
        let config = Config { api_key: " ".into(), ..config("claude-opus-5-5") };
        assert!(matches!(generate(&config, "", "", |_| {}), Err(AiError::MissingKey)));
    }
}
