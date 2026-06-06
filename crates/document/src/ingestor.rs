use crate::chunker::Chunker;
use crate::parser::extract_text;
use aios_memory::Database;
use aios_shared::Document;
use anyhow::Result;
use std::path::Path;
use tracing::{info, warn};
use walkdir::WalkDir;

pub struct Ingestor<'a> {
    db:      &'a Database,
    chunker: Chunker,
}

impl<'a> Ingestor<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self {
            db:      db,
            chunker: Chunker::default(),
        }
    }

    /// Ingest a single file — parse, chunk, store.
    pub fn ingest_file(&self, path: &Path) -> Result<Document> {
        let path_str = path.to_string_lossy().to_string();
        let size = std::fs::metadata(path)?.len() as i64;
        let mime = mime_for_path(path);
        let title = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(String::from);

        info!("Ingesting: {}", path_str);

        // Extract text
        let text = extract_text(path)?;
        if text.trim().is_empty() {
            anyhow::bail!("No text extracted from {}", path_str);
        }

        // Upsert document record
        let doc = self.db.upsert_document(
            &path_str,
            title.as_deref(),
            &mime,
            size,
        )?;

        // Delete old chunks for this document (re-index)
        self.db.delete_chunks_for_document(doc.id)?;

        // Chunk and store
        let chunks = self.chunker.chunk(&text);
        info!("  {} chunks from {} chars", chunks.len(), text.len());

        for (i, chunk) in chunks.iter().enumerate() {
            self.db.add_chunk(doc.id, chunk, i as i64)?;
        }

        Ok(doc)
    }

    /// Recursively ingest all supported files under a directory.
    pub fn ingest_directory(&self, dir: &Path) -> Result<Vec<Document>> {
        let mut docs = Vec::new();

        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            match self.ingest_file(entry.path()) {
                Ok(doc) => docs.push(doc),
                Err(e) => warn!("Skipping {}: {}", entry.path().display(), e),
            }
        }

        Ok(docs)
    }
}

fn mime_for_path(path: &Path) -> String {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "pdf"  => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "txt"  => "text/plain",
        "md"   => "text/markdown",
        _      => "application/octet-stream",
    }
    .to_string()
}