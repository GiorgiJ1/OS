use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error};

#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub base_url: String,
    pub model:    String,
    pub timeout:  Duration,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: std::env::var("OLLAMA_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            model: std::env::var("OLLAMA_MODEL")
                .unwrap_or_else(|_| "llama3.2".to_string()),
            timeout: Duration::from_secs(120),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role:    String,
    pub content: String,
}

// ── Ollama wire types ─────────────────────────────────────────────────────────

#[derive(Serialize)]
struct OllamaChatRequest<'a> {
    model:    &'a str,
    messages: &'a [ChatMessage],
    stream:   bool,
}

#[derive(Deserialize)]
struct OllamaChatChunk {
    message: Option<ChunkMessage>,
    done:    bool,
}

#[derive(Deserialize)]
struct ChunkMessage {
    content: String,
}

// ── Client ────────────────────────────────────────────────────────────────────

pub struct OllamaClient {
    config: OllamaConfig,
    http:   Client,
}

impl OllamaClient {
    pub fn new(config: OllamaConfig) -> Result<Self> {
        let http = Client::builder()
            .timeout(config.timeout)
            .build()?;
        Ok(Self { config, http })
    }

    pub fn with_defaults() -> Result<Self> {
        Self::new(OllamaConfig::default())
    }

    /// Non-streaming — returns the full response string.
    pub async fn chat(&self, messages: &[ChatMessage]) -> Result<String> {
        let url = format!("{}/api/chat", self.config.base_url);
        let body = OllamaChatRequest {
            model:    &self.config.model,
            messages,
            stream:   false,
        };

        debug!("Sending chat request to Ollama (model={})", self.config.model);

        let resp = self.http
            .post(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        let chunk: OllamaChatChunk = resp.json().await?;
        Ok(chunk.message.map(|m| m.content).unwrap_or_default())
    }

    /// Streaming — sends tokens over an mpsc channel as they arrive.
    /// Returns the full assembled response when the stream is done.
    pub async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tx: mpsc::Sender<String>,
    ) -> Result<String> {
        let url = format!("{}/api/chat", self.config.base_url);
        let body = OllamaChatRequest {
            model:    &self.config.model,
            messages,
            stream:   true,
        };

        debug!("Starting streaming chat (model={})", self.config.model);

        let resp = self.http
            .post(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        let mut stream = resp.bytes_stream();
        let mut full_response = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk?;
            let text = std::str::from_utf8(&bytes)?;

            for line in text.lines() {
                if line.is_empty() {
                    continue;
                }
                match serde_json::from_str::<OllamaChatChunk>(line) {
                    Ok(c) => {
                        if let Some(msg) = c.message {
                            full_response.push_str(&msg.content);
                            if tx.send(msg.content).await.is_err() {
                                // receiver dropped — caller cancelled
                                break;
                            }
                        }
                        if c.done {
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse Ollama chunk: {} — line: {}", e, line);
                    }
                }
            }
        }

        Ok(full_response)
    }

    /// Check if Ollama is reachable.
    pub async fn health_check(&self) -> bool {
        let url = format!("{}/api/tags", self.config.base_url);
        self.http.get(&url).send().await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}