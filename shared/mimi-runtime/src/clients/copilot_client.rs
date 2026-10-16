//! Independent, cancellable streaming Chat Completions client for copilot.

use crate::clients::provider_network::ProviderNetwork;
use crate::core::protocols::copilot_prompt::{
    self, CopilotPrompt, CopilotUsage, SseLines, StreamFrame,
};
use crate::core::protocols::openai_compatible::{self, OpenAICompatibleError};
use futures_util::StreamExt;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct CopilotClient {
    endpoint: url::Url,
    api_key: String,
    model: String,
    client: reqwest::Client,
}

pub struct CopilotCompletion {
    pub text: String,
    pub first_token_ms: Option<u64>,
    pub total_ms: u64,
    pub usage: Option<CopilotUsage>,
}

impl CopilotClient {
    pub fn new(
        endpoint: &str,
        api_key: &str,
        model: &str,
        network: ProviderNetwork,
    ) -> Result<Self, OpenAICompatibleError> {
        if api_key.len() > 4096
            || api_key.chars().any(char::is_control)
            || reqwest::header::HeaderValue::from_str(&format!("Bearer {api_key}")).is_err()
        {
            return Err(OpenAICompatibleError::APIKey);
        }
        let endpoint = openai_compatible::endpoint(endpoint)?;
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = network
            .http_client_builder(&endpoint)
            .map_err(|_| OpenAICompatibleError::Connection)?
            .build()
            .map_err(|_| OpenAICompatibleError::Connection)?;
        Ok(Self {
            endpoint,
            api_key: api_key.to_owned(),
            model: openai_compatible::validate_model(model)?,
            client,
        })
    }

    pub async fn stream(
        &self,
        prompt: CopilotPrompt<'_>,
        on_delta: impl FnMut(&str) + Send,
    ) -> Result<CopilotCompletion, OpenAICompatibleError> {
        let body = copilot_prompt::request(prompt, &self.model, &self.endpoint)?;
        self.complete(body, false, on_delta).await
    }

    pub async fn warm(&self, prefix: &str) -> Result<CopilotCompletion, OpenAICompatibleError> {
        let body = copilot_prompt::warmup_request(prefix, &self.model, &self.endpoint)?;
        self.complete(body, true, |_| {}).await
    }

    async fn complete(
        &self,
        body: serde_json::Value,
        warm: bool,
        mut on_delta: impl FnMut(&str) + Send,
    ) -> Result<CopilotCompletion, OpenAICompatibleError> {
        tokio::time::timeout(Duration::from_secs(8), async {
            let start = Instant::now();
            let mut request = self.client.post(self.endpoint.clone()).json(&body);
            if !self.api_key.is_empty() {
                request = request.bearer_auth(&self.api_key);
            }
            let response = request.send().await.map_err(|error| {
                if error.is_timeout() {
                    OpenAICompatibleError::Timeout
                } else {
                    OpenAICompatibleError::Connection
                }
            })?;
            if !response.status().is_success() {
                return Err(OpenAICompatibleError::Rejected(response.status().as_u16()));
            }
            if response
                .content_length()
                .is_some_and(|length| length > copilot_prompt::MAX_STREAM_BYTES as u64)
            {
                return Err(OpenAICompatibleError::TooLarge);
            }
            let mut chunks = response.bytes_stream();
            let mut parser = SseLines::default();
            let mut text = String::new();
            let mut first_token_ms = None;
            let mut usage = None;
            let mut done = false;
            let mut cut_line = false;
            while let Some(chunk) = chunks.next().await {
                let chunk = chunk.map_err(|_| OpenAICompatibleError::Connection)?;
                for event in parser.push(&chunk)? {
                    match copilot_prompt::decode_frame(&event)? {
                        StreamFrame::Delta(delta) => {
                            if !warm && !cut_line {
                                let segment = delta.split(['\n', '\r']).next().unwrap_or_default();
                                cut_line = segment.len() < delta.len();
                                if segment.len()
                                    > copilot_prompt::MAX_SUGGESTION_BYTES
                                        .saturating_sub(text.len())
                                {
                                    return Err(OpenAICompatibleError::TooLarge);
                                }
                                if !segment.is_empty() {
                                    first_token_ms
                                        .get_or_insert(start.elapsed().as_millis() as u64);
                                    text.push_str(segment);
                                    on_delta(segment);
                                }
                            }
                        }
                        StreamFrame::Usage(value) => usage = Some(value),
                        StreamFrame::Done => {
                            done = true;
                            break;
                        }
                        StreamFrame::Finished | StreamFrame::Ignore => {}
                    }
                }
                if done {
                    break;
                }
            }
            if !done || (!warm && text.trim().is_empty()) {
                return Err(OpenAICompatibleError::Response);
            }
            Ok(CopilotCompletion {
                text: text.trim().to_owned(),
                first_token_ms,
                total_ms: start.elapsed().as_millis() as u64,
                usage,
            })
        })
        .await
        .map_err(|_| OpenAICompatibleError::Timeout)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::network_proxy::{ProxyConfig, ProxyMode};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn stream_reads_chunked_sse_and_usage_without_exposing_error_body() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0; 4096];
            let read = socket.read(&mut buffer).await.unwrap();
            let request = String::from_utf8_lossy(&buffer[..read]);
            assert!(request.contains("POST /v1/chat/completions"));
            assert!(request.contains("Bearer synthetic-key"));
            let body = concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\r\n\r\n",
                ": keepalive\r\n\r\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\" there\"}}]}\r\n\r\n",
                "data: {\"usage\":{\"prompt_tokens\":20,\"completion_tokens\":3,\"prompt_cache_hit_tokens\":16,\"prompt_cache_miss_tokens\":4}}\r\n\r\n",
                "data: [DONE]\r\n\r\n"
            );
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).await.unwrap();
            for part in body.as_bytes().chunks(7) {
                socket.write_all(part).await.unwrap();
            }
        });
        let network = ProviderNetwork::resolve(&ProxyConfig {
            mode: ProxyMode::Direct,
            url: None,
        })
        .unwrap();
        let client = CopilotClient::new(
            &format!("http://{address}/v1"),
            "synthetic-key",
            "test-model",
            network,
        )
        .unwrap();
        let mut deltas = Vec::new();
        let result = client
            .stream(
                CopilotPrompt {
                    prefix: "fixed",
                    transcript: "Other: hello",
                },
                |delta| deltas.push(delta.to_owned()),
            )
            .await
            .unwrap();
        server.await.unwrap();
        assert_eq!(result.text, "Hello there");
        assert_eq!(deltas, ["Hello", " there"]);
        assert_eq!(result.usage.unwrap().cache_hit_tokens, Some(16));
    }

    #[tokio::test]
    async fn rejected_status_is_content_free() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0; 4096];
            let _ = socket.read(&mut buffer).await.unwrap();
            socket.write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 17\r\nConnection: close\r\n\r\nPRIVATE SECRET!!!").await.unwrap();
        });
        let network = ProviderNetwork::resolve(&ProxyConfig {
            mode: ProxyMode::Direct,
            url: None,
        })
        .unwrap();
        let client = CopilotClient::new(
            &format!("http://{address}/v1"),
            "synthetic-key",
            "test-model",
            network,
        )
        .unwrap();
        let result = client
            .stream(
                CopilotPrompt {
                    prefix: "fixed",
                    transcript: "Other: hello",
                },
                |_| {},
            )
            .await;
        server.await.unwrap();
        assert_eq!(result.err(), Some(OpenAICompatibleError::Rejected(401)));
    }
}
