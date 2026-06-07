use anyhow::Result;
use aios_memory::Database;
use aios_models::{OllamaClient, OllamaConfig, ChatMessage};
use aios_search::SearchEngine;
use aios_shared::{Conversation, Message, Role};
use tokio::sync::mpsc;
use tracing::info;
use uuid::Uuid;

pub struct Assistant {
    pub db:    Database,
    ollama:    OllamaClient,
    search:    SearchEngine,
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

    /// Build the system prompt from memories + document context.
    fn build_system_prompt(
        &self,
        memories: &[(String, String)],
        context:  &str,
    ) -> String {
        let mut system = String::from(
            "You are AIOS, an AI assistant integrated into the user's operating system. \
             You have access to the user's documents and remember facts across sessions.\n\n"
        );

        if !memories.is_empty() {
            system.push_str("What you know about the user:\n");
            for (key, value) in memories {
                system.push_str(&format!("- {}: {}\n", key, value));
            }
            system.push('\n');
        }

        if !context.is_empty() {
            system.push_str("Relevant context from documents:\n\n");
            system.push_str(context);
        }

        system
    }

    /// Extract key facts from the conversation and store them as memories.
    async fn extract_and_store_memories(
        &self,
        user_input: &str,
        response:   &str,
    ) -> Result<()> {
        let extraction_prompt = format!(
            "Extract any important facts about the user from this exchange. \
             Return ONLY a JSON object like {{\"key\": \"value\"}} for each fact. \
             If there are no important facts, return {{}}\n\
             Keys should be short snake_case strings like 'user_name', 'user_location', \
             'user_project', 'user_preference'.\n\n\
             User: {}\nAssistant: {}",
            user_input, response
        );

        let messages = vec![ChatMessage {
            role:    "user".to_string(),
            content: extraction_prompt,
        }];

        let raw = self.ollama.chat(&messages).await?;

        // Parse the JSON response
        let cleaned = raw
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        if let Ok(map) = serde_json::from_str::<serde_json::Value>(cleaned) {
            if let Some(obj) = map.as_object() {
                for (key, value) in obj {
                    if let Some(v) = value.as_str() {
                        if !v.is_empty() && v != "unknown" {
                            self.db.set_memory(key, v, Some("conversation"))?;
                            info!("Stored memory: {} = {}", key, v);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn chat_stream_with_context(
        &self,
        conversation_id: Uuid,
        user_input:      &str,
        tx:              mpsc::Sender<String>,
    ) -> Result<Message> {
        // 1. Load memories
        let memories = self.db.get_all_memories()?;

        // 2. Search for relevant document chunks
        let results = self.search.search(&self.db, user_input, 5).await?;
        let context = if results.is_empty() {
            String::new()
        } else {
            let mut ctx = String::new();
            for (i, r) in results.iter().enumerate() {
                ctx.push_str(&format!(
                    "[{}]\n{}\n\n",
                    i + 1,
                    r.chunk.content
                ));
            }
            ctx
        };

        // 3. Persist user message
        self.db.add_message(conversation_id, Role::User, user_input)?;

        // 4. Build full message list
        let history = self.db.get_messages(conversation_id)?;
        let mut ollama_messages: Vec<ChatMessage> = vec![
            ChatMessage {
                role:    "system".to_string(),
                content: self.build_system_prompt(&memories, &context),
            }
        ];
        for msg in &history {
            ollama_messages.push(ChatMessage {
                role:    msg.role.to_string(),
                content: msg.content.clone(),
            });
        }

        info!(
            "Sending {} messages | {} memories | {} context chunks",
            ollama_messages.len(),
            memories.len(),
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

        // 7. Extract memories from this exchange (fire and forget)
        let _ = self.extract_and_store_memories(user_input, &full_response).await;

        Ok(msg)
    }

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