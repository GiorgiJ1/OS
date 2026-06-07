use crate::keyword::KeywordIndex;
use crate::semantic::semantic_search;
use aios_embeddings::EmbeddingEngine;
use aios_memory::Database;
use aios_shared::Chunk;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk:  Chunk,
    pub score:  f32,
    pub source: ResultSource,
}

#[derive(Debug, Clone)]
pub enum ResultSource {
    Semantic,
    Keyword,
    Both,
}

pub struct SearchEngine {
    keyword:   KeywordIndex,
    embedder:  EmbeddingEngine,
    model:     String,
}

impl SearchEngine {
    pub fn new<P: AsRef<Path>>(index_path: P) -> Result<Self> {
        let model = std::env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "nomic-embed-text".to_string());
        Ok(Self {
            keyword:  KeywordIndex::open(index_path)?,
            embedder: EmbeddingEngine::with_defaults()?,
            model,
        })
    }

    /// Index a chunk into Tantivy for keyword search.
    pub fn index_chunk(&self, chunk_id: Uuid, content: &str) -> Result<()> {
        self.keyword.index_chunk(chunk_id, content)
    }

    /// Run hybrid search: semantic + keyword, merge and rank results.
    pub async fn search(
        &self,
        db:      &Database,
        query:   &str,
        top_k:   usize,
    ) -> Result<Vec<SearchResult>> {
        // 1. Embed the query
        let query_vector = self.embedder.embed_query(query).await?;

        // 2. Semantic search over stored vectors
        let semantic = semantic_search(db, &query_vector, &self.model, top_k)?;

        // 3. Keyword search via Tantivy
        let keyword = self.keyword.search(query, top_k).unwrap_or_default();

        // 4. Merge scores into a map keyed by chunk_id
        // Semantic scores are already 0-1. Keyword scores are TF-IDF,
        // so we normalise them to 0-1 by dividing by the max score.
        let kw_max = keyword.iter().map(|(_, s)| *s).fold(0.0_f32, f32::max);

        let mut scores: HashMap<Uuid, (f32, ResultSource)> = HashMap::new();

        for r in &semantic {
            scores.insert(r.chunk_id, (r.score, ResultSource::Semantic));
        }

        for (chunk_id, kw_score) in &keyword {
            let normalised = if kw_max > 0.0 { kw_score / kw_max } else { 0.0 };
            scores
                .entry(*chunk_id)
                .and_modify(|(s, src)| {
                    *s = (*s + normalised) / 2.0; // average when both hit
                    *src = ResultSource::Both;
                })
                .or_insert((normalised, ResultSource::Keyword));
        }

        // 5. Fetch chunk content and assemble results
        let mut chunk_ids: Vec<Uuid> = scores.keys().cloned().collect();
        chunk_ids.sort_by(|a, b| {
            scores[b].0.partial_cmp(&scores[a].0).unwrap()
        });
        chunk_ids.truncate(top_k);

        let chunks = db.get_chunks_by_ids(&chunk_ids)?;
        let mut results: Vec<SearchResult> = chunks
            .into_iter()
            .map(|chunk| {
                let (score, source) = scores[&chunk.id].clone();
                SearchResult { chunk, score, source }
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        info!("Hybrid search '{}' → {} results", query, results.len());
        Ok(results)
    }
}