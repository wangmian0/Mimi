//! Fixed-prefix, bounded Chat Completions protocol for the optional copilot.

use super::openai_compatible::{self, OpenAICompatibleError};
use serde_json::{json, Value};

pub const MAX_PROMPT_BYTES: usize = 96 * 1024;
pub const MAX_STREAM_BYTES: usize = 256 * 1024;
pub const MAX_SUGGESTION_BYTES: usize = 2 * 1024;
const MAX_TOKENS: u32 = 96;
const WARMUP_TRANSCRIPT: &str = "Prepare to suggest the next reply.";

pub fn prefix_fits(prefix: &str) -> bool {
    prefix.len().saturating_add(WARMUP_TRANSCRIPT.len()) <= MAX_PROMPT_BYTES
}

#[derive(Clone, Copy)]
pub struct CopilotPrompt<'a> {
    pub prefix: &'a str,
    pub transcript: &'a str,
}

pub fn request(
    prompt: CopilotPrompt<'_>,
    model: &str,
    endpoint: &url::Url,
) -> Result<Value, OpenAICompatibleError> {
    body(
        prompt.prefix,
        prompt.transcript,
        model,
        endpoint,
        MAX_TOKENS,
    )
}

pub fn warmup_request(
    prefix: &str,
    model: &str,
    endpoint: &url::Url,
) -> Result<Value, OpenAICompatibleError> {
    body(prefix, WARMUP_TRANSCRIPT, model, endpoint, 1)
}

fn body(
    prefix: &str,
    transcript: &str,
    model: &str,
    endpoint: &url::Url,
    max_tokens: u32,
) -> Result<Value, OpenAICompatibleError> {
    if prefix.len().saturating_add(transcript.len()) > MAX_PROMPT_BYTES {
        return Err(OpenAICompatibleError::TooLarge);
    }
    let mut value = json!({
        "model": openai_compatible::validate_model(model)?,
        "stream": true,
        "stream_options": {"include_usage": true},
        "max_tokens": max_tokens,
        "stop": ["\n"],
        "messages": [
            {"role": "system", "content": prefix},
            {"role": "user", "content": transcript}
        ]
    });
    super::translation_thinking::disable(&mut value, endpoint);
    Ok(value)
}

#[derive(Default)]
pub struct SseLines {
    pending: Vec<u8>,
    data: Vec<String>,
    size: usize,
}

impl SseLines {
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<String>, OpenAICompatibleError> {
        self.size = self.size.saturating_add(chunk.len());
        if self.size > MAX_STREAM_BYTES
            || self.pending.len().saturating_add(chunk.len()) > MAX_STREAM_BYTES
        {
            return Err(OpenAICompatibleError::TooLarge);
        }
        self.pending.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(index) = self.pending.iter().position(|byte| *byte == b'\n') {
            let mut line = self.pending.drain(..=index).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.is_empty() {
                if !self.data.is_empty() {
                    events.push(self.data.join("\n"));
                    self.data.clear();
                }
                continue;
            }
            if line[0] == b':' {
                continue;
            }
            if let Some(data) = line.strip_prefix(b"data:") {
                let data = data.strip_prefix(b" ").unwrap_or(data);
                self.data.push(
                    String::from_utf8(data.to_vec())
                        .map_err(|_| OpenAICompatibleError::Response)?,
                );
            }
        }
        Ok(events)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CopilotUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cache_hit_tokens: Option<u32>,
    pub cache_miss_tokens: Option<u32>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum StreamFrame {
    Delta(String),
    Finished,
    Usage(CopilotUsage),
    Done,
    Ignore,
}

pub fn decode_frame(data: &str) -> Result<StreamFrame, OpenAICompatibleError> {
    if data == "[DONE]" {
        return Ok(StreamFrame::Done);
    }
    let value: Value = serde_json::from_str(data).map_err(|_| OpenAICompatibleError::Response)?;
    if let Some(usage) = value.get("usage").filter(|usage| !usage.is_null()) {
        let num = |key: &str| {
            usage
                .get(key)
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
        };
        let details = usage.get("prompt_tokens_details");
        return Ok(StreamFrame::Usage(CopilotUsage {
            prompt_tokens: num("prompt_tokens").unwrap_or(0),
            completion_tokens: num("completion_tokens").unwrap_or(0),
            cache_hit_tokens: num("prompt_cache_hit_tokens").or_else(|| {
                details
                    .and_then(|d| d.get("cached_tokens"))
                    .and_then(Value::as_u64)
                    .and_then(|n| u32::try_from(n).ok())
            }),
            cache_miss_tokens: num("prompt_cache_miss_tokens"),
        }));
    }
    let Some(choice) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
    else {
        return Ok(StreamFrame::Ignore);
    };
    let finish = choice
        .get("finish_reason")
        .filter(|reason| !reason.is_null());
    if finish.is_some_and(|reason| !matches!(reason.as_str(), Some("stop" | "length"))) {
        return Err(OpenAICompatibleError::Response);
    }
    match choice
        .get("delta")
        .and_then(|delta| delta.get("content"))
        .and_then(Value::as_str)
    {
        Some(text) if !text.is_empty() => Ok(StreamFrame::Delta(text.to_owned())),
        _ if finish.is_some() => Ok(StreamFrame::Finished),
        _ => Ok(StreamFrame::Ignore),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_is_byte_stable_when_conversation_grows() {
        let endpoint = openai_compatible::endpoint("https://api.deepseek.com/v1").unwrap();
        let prefix = "Rules\nPrompt: café\nBackground: A\nStrategy: B";
        let first = request(
            CopilotPrompt {
                prefix,
                transcript: "Other: Hello",
            },
            "deepseek-chat",
            &endpoint,
        )
        .unwrap();
        let second = request(
            CopilotPrompt {
                prefix,
                transcript: "Other: Hello\nMe: Hi",
            },
            "deepseek-chat",
            &endpoint,
        )
        .unwrap();
        assert_eq!(first["messages"][0], second["messages"][0]);
        assert!(second["messages"][1]["content"]
            .as_str()
            .unwrap()
            .starts_with(first["messages"][1]["content"].as_str().unwrap()));
        assert_eq!(first["thinking"]["type"], "disabled");
        assert_eq!(first["stream_options"]["include_usage"], true);
        assert_eq!(first["stop"][0], "\n");
        assert_eq!(
            warmup_request(prefix, "deepseek-chat", &endpoint).unwrap()["max_tokens"],
            1
        );
    }

    #[test]
    fn warmup_and_suggestion_share_the_prompt_budget() {
        let endpoint = openai_compatible::endpoint("https://api.deepseek.com/v1").unwrap();
        let prefix = "x".repeat(MAX_PROMPT_BYTES - WARMUP_TRANSCRIPT.len());
        assert!(prefix_fits(&prefix));
        assert!(warmup_request(&prefix, "deepseek-chat", &endpoint).is_ok());
        assert!(!prefix_fits(&(prefix + "x")));
        assert_eq!(
            warmup_request(&"x".repeat(MAX_PROMPT_BYTES), "deepseek-chat", &endpoint),
            Err(OpenAICompatibleError::TooLarge)
        );
    }

    #[test]
    fn sse_handles_fragments_crlf_keepalive_done_and_both_usage_shapes() {
        let mut parser = SseLines::default();
        assert!(parser.push(b": keepalive\r\nda").unwrap().is_empty());
        let frames = parser
            .push(b"ta: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\r\n\r\ndata: [DONE]\n\n")
            .unwrap();
        assert_eq!(
            decode_frame(&frames[0]).unwrap(),
            StreamFrame::Delta("Hi".into())
        );
        assert_eq!(decode_frame(&frames[1]).unwrap(), StreamFrame::Done);
        assert_eq!(
            decode_frame(r#"{"choices":[{"delta":{"content":"tail"},"finish_reason":"stop"}]}"#)
                .unwrap(),
            StreamFrame::Delta("tail".into())
        );
        assert_eq!(
            decode_frame(r#"{"choices":[{"delta":{},"finish_reason":"content_filter"}]}"#),
            Err(OpenAICompatibleError::Response)
        );
        for (json, hit, miss) in [
            (
                r#"{"usage":{"prompt_tokens":20,"completion_tokens":3,"prompt_cache_hit_tokens":16,"prompt_cache_miss_tokens":4}}"#,
                Some(16),
                Some(4),
            ),
            (
                r#"{"usage":{"prompt_tokens":20,"completion_tokens":3,"prompt_tokens_details":{"cached_tokens":12}}}"#,
                Some(12),
                None,
            ),
        ] {
            match decode_frame(json).unwrap() {
                StreamFrame::Usage(u) => {
                    assert_eq!(u.cache_hit_tokens, hit);
                    assert_eq!(u.cache_miss_tokens, miss);
                }
                _ => panic!("usage expected"),
            }
        }
    }
}
