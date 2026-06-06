use aios_memory::Database;
use aios_shared::Chunk;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};
use uuid::Uuid;

#[derive(Serialize)]
struct EmbedRequest<'a> {
    model:  &'a str,
    prompt: &'a str,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embedding: Vec<f64>,
}

pub struct EmbeddingEngine {
    base_url: String,
    model:    String,
    http:     Client,
}

impl EmbeddingEngine {
    pub fn new(base_url: &str, model: &str) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;
        Ok(Self {
            base_url: base_url.to_string(),
            model:    model.to_string(),
            http,
        })
    }

    pub fn with_defaults() -> Result<Self> {
        let base_url = std::env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let model = std::env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "nomic-embed-text".to_string());
        Self::new(&base_url, &model)
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.base_url);
        let body = EmbedRequest {
            model:  &self.model,
            prompt: text,
        };

        debug!("Embedding {} chars with model {}", text.len(), self.model);

        let resp = self.http
            .post(&url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        let data: EmbedResponse = resp.json().await?;
        Ok(data.embedding.iter().map(|&x| x as f32).collect())
    }

    pub async fn embed_document_chunks(
        &self,
        db:          &Database,
        document_id: Uuid,
    ) -> Result<usize> {
        let chunks = db.get_chunks_for_document(document_id)?;
        let mut count = 0;

        for chunk in &chunks {
            if db.embedding_exists(chunk.id, &self.model)? {
                debug!("Chunk {} already embedded, skipping", chunk.id);
                continue;
            }

            let vector = self.embed(&chunk.content).await?;
            db.store_embedding(chunk.id, &self.model, &vector)?;
            count += 1;
        }

        info!(
            "Embedded {}/{} chunks for document {}",
            count,
            chunks.len(),
            document_id
        );
        Ok(count)
    }

    pub async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        self.embed(query).await
    }
}