use anyhow::Result;
use aios_memory::Database;
use aios_models::{OllamaClient, OllamaConfig, ChatMessage};
use aios_search::SearchEngine;
use aios_shared::{Conversation, Message, Role};
use aios_skills::{SkillRegistry, default_registry};
use tokio::sync::mpsc;
use tracing::info;
use uuid::Uuid;

pub struct Assistant {
    pub db:    Database,
    ollama:    OllamaClient,
    search:    SearchEngine,
    skills:    SkillRegistry,
}

impl Assistant {
    pub fn new(db: Database, config: OllamaConfig, index_path: &str) -> Result<Self> {
        let ollama = OllamaClient::new(config)?;
        let search = SearchEngine::new(index_path)?;
        let skills = default_registry();
        Ok(Self { db, ollama, search, skills })
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

    /// When the user asks to write/save "this response" without pasting content,
    /// attach the previous assistant message so file_write can use it.
    fn augment_skill_input(
        &self,
        conversation_id: Uuid,
        user_input:      &str,
    ) -> String {
        let lower = user_input.to_lowercase();
        let references_previous = lower.contains("this response")
            || lower.contains("this responce")
            || lower.contains("previous response")
            || lower.contains("the above")
            || lower.contains("what you said")
            || lower.contains("your answer")
            || lower.contains("your response")
            || lower.contains("that answer")
            || lower.contains("that response");
        let is_write = lower.contains("write")
            || lower.contains("save")
            || lower.contains("put it in")
            || lower.contains("into the file");

        if !references_previous || !is_write {
            return user_input.to_string();
        }

        let Ok(history) = self.db.get_messages(conversation_id) else {
            return user_input.to_string();
        };

        if let Some(last) = history
            .iter()
            .rev()
            .find(|m| m.role == Role::Assistant)
        {
            return format!(
                "{user_input}\n\nContent to write:\n{}",
                last.content
            );
        }

        user_input.to_string()
    }

    fn build_system_prompt(
        &self,
        memories: &[(String, String)],
        context:  &str,
    ) -> String {
        let mut system = String::from(
            "You are Skvanchi, a personal AI assistant running locally on the user's machine.\n\
            CRITICAL RULES:\n\
            1. If 'Results from tools' appears below, you MUST answer using that data. \
               Web search results are real and current — summarize them for the user. \
               Do NOT say you cannot search the web when tool results are provided.\n\
            2. If a tool reports failure, tell the user the search/write failed and include the error.\n\
            3. If 'From indexed documents' appears below, quote from it directly.\n\
            4. Never tell the user to search elsewhere if tool results are provided.\n\
            5. Answer directly and concisely from the provided context.\n\n"
        );

        if !memories.is_empty() {
            system.push_str("What you know about the user:\n");
            for (key, value) in memories {
                system.push_str(&format!("- {}: {}\n", key, value));
            }
            system.push('\n');
        }

        if !context.is_empty() {
            system.push_str("=== CONTEXT (use this to answer) ===\n\n");
            system.push_str(context);
            system.push_str("\n=== END CONTEXT ===\n\n");
            system.push_str("You MUST answer from the context above. Do not ignore it.\n");
        }

        system
    }

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

        // 2. Run skills
        let skill_input = self.augment_skill_input(conversation_id, user_input);
        let skill_outputs: Vec<aios_skills::SkillOutput> =
            self.skills.execute_matching(&skill_input).await;
        let skill_context = SkillRegistry::format_outputs(&skill_outputs);
        let has_web_results = skill_outputs.iter().any(|o| o.success && o.label.starts_with("web:"));

        // 3. Search documents — skip when web search already returned results
        let results = if has_web_results {
            vec![]
        } else {
            self.search.search(&self.db, user_input, 5).await?
        };
        let doc_context = if results.is_empty() {
            String::new()
        } else {
            let mut ctx = String::new();
            for (i, r) in results.iter().enumerate() {
                // Only inject chunks with score above 0.4
                if r.score < 0.4 {
                    continue;
                }
                ctx.push_str(&format!("[{}]\n{}\n\n", i + 1, r.chunk.content));
            }
            ctx
        };

        // 4. Combine contexts
        let mut full_context = String::new();
        if !skill_context.is_empty() {
            full_context.push_str(&skill_context);
        }
        if !doc_context.is_empty() {
            full_context.push_str("From indexed documents:\n\n");
            full_context.push_str(&doc_context);
        }

        // 5. Persist user message
        self.db.add_message(conversation_id, Role::User, user_input)?;

        // 6. Build messages
        let history = self.db.get_messages(conversation_id)?;
        let mut ollama_messages: Vec<ChatMessage> = vec![
            ChatMessage {
                role:    "system".to_string(),
                content: self.build_system_prompt(&memories, &full_context),
            }
        ];
        for msg in &history {
            ollama_messages.push(ChatMessage {
                role:    msg.role.to_string(),
                content: msg.content.clone(),
            });
        }

        info!(
            "Sending {} messages | {} memories | {} skill results | {} doc chunks",
            ollama_messages.len(),
            memories.len(),
            skill_outputs.len(),
            results.len(),
        );

        // 7. Stream response
        let full_response = self.ollama
            .chat_stream(&ollama_messages, tx)
            .await?;

        // 8. Persist response
        let msg = self.db.add_message(
            conversation_id,
            Role::Assistant,
            &full_response,
        )?;

        // 9. Extract memories
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

    pub async fn ollama_chat(&self, messages: &[aios_models::ChatMessage]) -> anyhow::Result<String> {
        self.ollama.chat(messages).await
    }
}