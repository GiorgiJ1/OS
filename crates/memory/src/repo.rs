use crate::db::Database;
use aios_shared::*;
use anyhow::Result;
use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

impl Database {
    pub fn create_conversation(&self, title: Option<&str>) -> Result<Conversation> {
        let now = Utc::now();
        let conv = Conversation {
            id:         Uuid::new_v4(),
            title:      title.map(String::from),
            created_at: now,
            updated_at: now,
        };
        self.conn.execute(
            "INSERT INTO conversations (id, title, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                conv.id.to_string(),
                conv.title,
                conv.created_at.to_rfc3339(),
                conv.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(conv)
    }
    pub fn store_embedding(
        &self,
        chunk_id: uuid::Uuid,
        model:    &str,
        vector:   &[f32],
    ) -> Result<()> 
    {
        let bytes: Vec<u8> = vector_to_bytes(vector);
        self.conn.execute(
            "INSERT INTO embeddings (id, chunk_id, model, vector)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(chunk_id, model) DO UPDATE SET vector = excluded.vector",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                chunk_id.to_string(),
                model,
                bytes,
            ],
        )?;
        Ok(())
    }

    /// Check whether a chunk already has an embedding for this model.
    pub fn embedding_exists(&self, chunk_id: uuid::Uuid, model: &str) -> Result<bool> {
        let mut stmt = self.conn.prepare(
            "SELECT 1 FROM embeddings WHERE chunk_id = ?1 AND model = ?2 LIMIT 1",
        )?;
        let mut rows = stmt.query(rusqlite::params![chunk_id.to_string(), model])?;
        Ok(rows.next()?.is_some())
    }

    /// Load all embeddings for similarity search.
    /// Returns (chunk_id, vector) pairs.
    pub fn get_all_embeddings(&self, model: &str) -> Result<Vec<(uuid::Uuid, Vec<f32>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT chunk_id, vector FROM embeddings WHERE model = ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![model], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Vec<u8>>(1)?,
            ))
        })?;
        rows.map(|r| {
            let (id_str, bytes) = r?;
            let id = uuid::Uuid::parse_str(&id_str)?;
            let vector = bytes_to_vector(&bytes);
            Ok((id, vector))
        })
        .collect()
    }

    /// Get chunks by their IDs — used to retrieve results after search.
    pub fn get_chunks_by_ids(&self, ids: &[uuid::Uuid]) -> Result<Vec<aios_shared::Chunk>> {
        let mut results = Vec::new();
        for id in ids {
            let mut stmt = self.conn.prepare(
                "SELECT id, document_id, content, chunk_index, created_at
                FROM chunks WHERE id = ?1",
            )?;
            let mut rows = stmt.query(rusqlite::params![id.to_string()])?;
            if let Some(row) = rows.next()? {
                results.push(aios_shared::Chunk {
                    id:          uuid::Uuid::parse_str(&row.get::<_, String>(0)?)?,
                    document_id: uuid::Uuid::parse_str(&row.get::<_, String>(1)?)?,
                    content:     row.get(2)?,
                    chunk_index: row.get(3)?,
                    created_at:  chrono::DateTime::parse_from_rfc3339(
                                    &row.get::<_, String>(4)?
                                )?.with_timezone(&chrono::Utc),
                });
            }
        }
        Ok(results)
    }

    pub fn list_conversations(&self) -> Result<Vec<Conversation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, created_at, updated_at
             FROM conversations ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|r| {
            let (id, title, ca, ua) = r?;
            Ok(Conversation {
                id:         Uuid::parse_str(&id)?,
                title,
                created_at: chrono::DateTime::parse_from_rfc3339(&ca)?.with_timezone(&Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(&ua)?.with_timezone(&Utc),
            })
        })
        .collect()
    }

    pub fn add_message(
        &self,
        conversation_id: Uuid,
        role: Role,
        content: &str,
    ) -> Result<Message> {
        let msg = Message {
            id:              Uuid::new_v4(),
            conversation_id,
            role,
            content:         content.to_string(),
            created_at:      Utc::now(),
        };
        self.conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                msg.id.to_string(),
                msg.conversation_id.to_string(),
                msg.role.to_string(),
                msg.content,
                msg.created_at.to_rfc3339(),
            ],
        )?;
        self.conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), conversation_id.to_string()],
        )?;
        Ok(msg)
    }

    pub fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, conversation_id, role, content, created_at
             FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![conversation_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.map(|r| {
            let (id, conv_id, role, content, ca) = r?;
            Ok(Message {
                id:              Uuid::parse_str(&id)?,
                conversation_id: Uuid::parse_str(&conv_id)?,
                role:            role.parse()?,
                content,
                created_at:      chrono::DateTime::parse_from_rfc3339(&ca)?.with_timezone(&Utc),
            })
        })
        .collect()
    }

    pub fn upsert_document(
        &self,
        path: &str,
        title: Option<&str>,
        mime_type: &str,
        size_bytes: i64,
    ) -> Result<Document> {
        let now = Utc::now();
        let doc = Document {
            id:         Uuid::new_v4(),
            path:       path.to_string(),
            title:      title.map(String::from),
            mime_type:  mime_type.to_string(),
            size_bytes,
            indexed_at: now,
            created_at: now,
        };
        self.conn.execute(
            "INSERT INTO documents (id, path, title, mime_type, size_bytes, indexed_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(path) DO UPDATE SET
                 title      = excluded.title,
                 mime_type  = excluded.mime_type,
                 size_bytes = excluded.size_bytes,
                 indexed_at = excluded.indexed_at",
            params![
                doc.id.to_string(),
                doc.path,
                doc.title,
                doc.mime_type,
                doc.size_bytes,
                doc.indexed_at.to_rfc3339(),
                doc.created_at.to_rfc3339(),
            ],
        )?;
        Ok(doc)
    }

    pub fn add_chunk(
        &self,
        document_id: Uuid,
        content: &str,
        chunk_index: i64,
    ) -> Result<Chunk> {
        let chunk = Chunk {
            id:          Uuid::new_v4(),
            document_id,
            content:     content.to_string(),
            chunk_index,
            created_at:  Utc::now(),
        };
        self.conn.execute(
            "INSERT INTO chunks (id, document_id, content, chunk_index, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                chunk.id.to_string(),
                chunk.document_id.to_string(),
                chunk.content,
                chunk.chunk_index,
                chunk.created_at.to_rfc3339(),
            ],
        )?;
        Ok(chunk)
    }

    pub fn get_chunks_for_document(&self, document_id: Uuid) -> Result<Vec<Chunk>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, document_id, content, chunk_index, created_at
             FROM chunks WHERE document_id = ?1 ORDER BY chunk_index ASC",
        )?;
        let rows = stmt.query_map(params![document_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.map(|r| {
            let (id, doc_id, content, idx, ca) = r?;
            Ok(Chunk {
                id:          Uuid::parse_str(&id)?,
                document_id: Uuid::parse_str(&doc_id)?,
                content,
                chunk_index: idx,
                created_at:  chrono::DateTime::parse_from_rfc3339(&ca)?.with_timezone(&Utc),
            })
        })
        .collect()
    }

    pub fn set_memory(&self, key: &str, value: &str, source: Option<&str>) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO memories (id, key, value, source, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(key) DO UPDATE SET
                 value      = excluded.value,
                 source     = excluded.source,
                 updated_at = excluded.updated_at",
            params![Uuid::new_v4().to_string(), key, value, source, now, now],
        )?;
        Ok(())
    }

    pub fn get_memory(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn
            .prepare("SELECT value FROM memories WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        Ok(rows.next()?.map(|r| r.get::<_, String>(0)).transpose()?)
    }

    pub fn delete_chunks_for_document(&self, document_id: uuid::Uuid) -> Result<()> {
    self.conn.execute(
        "DELETE FROM chunks WHERE document_id = ?1",
        rusqlite::params![document_id.to_string()],
    )?;
    Ok(())
    }
}

pub fn vector_to_bytes(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn bytes_to_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_and_messages() -> Result<()> {
        let db = Database::open_in_memory()?;
        let conv = db.create_conversation(Some("Test"))?;
        db.add_message(conv.id, Role::User, "Hello")?;
        db.add_message(conv.id, Role::Assistant, "Hi!")?;
        let msgs = db.get_messages(conv.id)?;
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, Role::User);
        Ok(())
    }

    #[test]
    fn test_document_and_chunks() -> Result<()> {
        let db = Database::open_in_memory()?;
        let doc = db.upsert_document("/tmp/test.pdf", Some("Test"), "application/pdf", 1024)?;
        db.add_chunk(doc.id, "First chunk", 0)?;
        db.add_chunk(doc.id, "Second chunk", 1)?;
        let chunks = db.get_chunks_for_document(doc.id)?;
        assert_eq!(chunks.len(), 2);
        Ok(())
    }

    #[test]
    fn test_memory_upsert() -> Result<()> {
        let db = Database::open_in_memory()?;
        db.set_memory("name", "AIOS", Some("user"))?;
        db.set_memory("name", "Updated", Some("user"))?;
        assert_eq!(db.get_memory("name")?, Some("Updated".to_string()));
        Ok(())
    }
}