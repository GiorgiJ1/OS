use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role {
    User,
    Assistant,
    System,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::User      => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::System    => write!(f, "system"),
        }
    }
}

impl std::str::FromStr for Role {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user"      => Ok(Role::User),
            "assistant" => Ok(Role::Assistant),
            "system"    => Ok(Role::System),
            other       => Err(anyhow::anyhow!("Unknown role: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id:         Uuid,
    pub title:      Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id:              Uuid,
    pub conversation_id: Uuid,
    pub role:            Role,
    pub content:         String,
    pub created_at:      DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id:         Uuid,
    pub path:       String,
    pub title:      Option<String>,
    pub mime_type:  String,
    pub size_bytes: i64,
    pub indexed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id:          Uuid,
    pub document_id: Uuid,
    pub content:     String,
    pub chunk_index: i64,
    pub created_at:  DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    pub id:       Uuid,
    pub chunk_id: Uuid,
    pub model:    String,
    pub vector:   Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id:          Uuid,
    pub name:        String,
    pub description: Option<String>,
    pub created_at:  DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id:         Uuid,
    pub project_id: Option<Uuid>,
    pub title:      String,
    pub status:     TaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Todo,
    InProgress,
    Done,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Todo       => write!(f, "todo"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Done       => write!(f, "done"),
        }
    }
}