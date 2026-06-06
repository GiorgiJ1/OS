use anyhow::Result;
use aios_memory::Database;
use aios_models::{OllamaClient, OllamaConfig, ChatMessage};
use aios_shared::{Conversation, Message, Role};
use tokio::sync::mpsc;
use tracing::info;
use uuid::Uuid;

pub struct Assistant {
    db:     Database,
    ollama: OllamaClient,
}

impl Assistant {
    pub fn new(db: Database, config: OllamaConfig) -> Result<Self> {
        let ollama = OllamaClient::new(config)?;
        Ok(Self { db, ollama })
    }

    pub fn with_defaults(db: Database) -> Result<Self> {
        Self::new(db, OllamaConfig::default())
    }

    /// Start a new conversation and return it.
    pub fn new_conversation(&self, title: Option<&str>) -> Result<Conversation> {
        self.db.create_conversation(title)
    }

    /// Send a user message, get a streamed response.
    /// Tokens arrive on `tx` as they stream; the full response is persisted
    /// to the DB and returned when complete.
    pub async fn chat_stream(
        &self,
        conversation_id: Uuid,
        user_input: &str,
        tx: mpsc::Sender<String>,
    ) -> Result<Message> {
        // 1. Persist user message
        self.db.add_message(conversation_id, Role::User, user_input)?;

        // 2. Build message history for context window
        let history = self.db.get_messages(conversation_id)?;
        let ollama_messages: Vec<ChatMessage> = history
            .iter()
            .map(|m| ChatMessage {
                role:    m.role.to_string(),
                content: m.content.clone(),
            })
            .collect();

        info!(
            "Sending {} messages to Ollama for conversation {}",
            ollama_messages.len(),
            conversation_id
        );

        // 3. Stream from Ollama
        let full_response = self.ollama
            .chat_stream(&ollama_messages, tx)
            .await?;

        // 4. Persist assistant response
        let msg = self.db.add_message(
            conversation_id,
            Role::Assistant,
            &full_response,
        )?;

        Ok(msg)
    }

    /// Non-streaming version — useful for background tasks and tests.
    pub async fn chat(
        &self,
        conversation_id: Uuid,
        user_input: &str,
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

    /// Quick health check — is Ollama running?
    pub async fn is_ready(&self) -> bool {
        self.ollama.health_check().await
    }

    pub fn db(&self) -> &Database {
    &self.db
    }
}