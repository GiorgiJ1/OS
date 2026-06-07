use anyhow::Result;
use aios_memory::Database;
use aios_models::{OllamaClient, OllamaConfig, ChatMessage};
use aios_search::SearchEngine;
use aios_shared::{Conversation, Message, Role};
use tokio::sync::mpsc;
use tracing::info;
use uuid::Uuid;

pub struct Assistant {
    pub db:     Database,
    ollama:     OllamaClient,
    search:     SearchEngine,
}

impl Assistant {
    pub fn new(db: Database, config: OllamaConfig, index_path: &str) -> Result<Self> {
        let ollama = OllamaClient::new(config)?;
        let search = SearchEngine::new(index_path)?;
        Ok(Self { db, ollama, search })
    }

    pub fn with_defaults(db: Database) -> Result<Self> {
        let data_dir = std::env::var("AIOS_DATA_DIR").unwrap_or_else(|_| {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".to_string());
            format!("{}/.local/share/aios", home)
        });
        let index_path = format!("{}/tantivy", data_dir);
        Self::new(db, OllamaConfig::default(), &index_path)
    }

    pub fn db(&self) -> &Database {
        &self.db
    }

    pub fn search_engine(&self) -> &SearchEngine {
        &self.search
    }

    pub fn new_conversation(&self, title: Option<&str>) -> Result<Conversation> {
        self.db.create_conversation(title)
    }

    /// Search documents and inject relevant context into the prompt.
    pub async fn chat_stream_with_context(
        &self,
        conversation_id: Uuid,
        user_input:      &str,
        tx:              mpsc::Sender<String>,
    ) -> Result<Message> {
        // 1. Search for relevant chunks
        let results = self.search.search(&self.db, user_input, 5).await?;

        // 2. Build context block from top results
        let context = if results.is_empty() {
            String::new()
        } else {
            let mut ctx = String::from("Relevant context from your documents:\n\n");
            for (i, r) in results.iter().enumerate() {
                ctx.push_str(&format!(
                    "[{}] (score: {:.2})\n{}\n\n",
                    i + 1,
                    r.score,
                    r.chunk.content
                ));
            }
            ctx
        };

        // 3. Persist user message
        self.db.add_message(conversation_id, Role::User, user_input)?;

        // 4. Build message history with context injected
        let history = self.db.get_messages(conversation_id)?;
        let mut ollama_messages: Vec<ChatMessage> = Vec::new();

        // System message with context
        if !context.is_empty() {
            ollama_messages.push(ChatMessage {
                role:    "system".to_string(),
                content: format!(
                    "You are AIOS, an AI assistant with access to the user's documents.\
                     Answer using the provided context when relevant.\n\n{}",
                    context
                ),
            });
        }

        // Conversation history
        for msg in &history {
            ollama_messages.push(ChatMessage {
                role:    msg.role.to_string(),
                content: msg.content.clone(),
            });
        }

        info!(
            "Sending {} messages with {} context chunks",
            ollama_messages.len(),
            results.len()
        );

        // 5. Stream response
        let full_response = self.ollama
            .chat_stream(&ollama_messages, tx)
            .await?;

        // 6. Persist assistant response
        let msg = self.db.add_message(
            conversation_id,
            Role::Assistant,
            &full_response,
        )?;

        Ok(msg)
    }

    /// Non-streaming fallback.
    pub async fn chat(
        &self,
        conversation_id: Uuid,
        user_input:      &str,
    ) -> Result<Message> {
        self.db.add_message(conversation_id, Role::User, user_input)?;
        let history = self.db.get_messages(conversation_id)?;
        let ollama_messages: Vec<ChatMessage> = history
            .iter()
            .map(|m| ChatMessage {
                role:    m.role.to_string(),
                content: m.content.clone(),
            })
            .collect();
        let response = self.ollama.chat(&ollama_messages).await?;
        let msg = self.db.add_message(conversation_id, Role::Assistant, &response)?;
        Ok(msg)
    }

    pub async fn is_ready(&self) -> bool {
        self.ollama.health_check().await
    }
}