use anyhow::Result;
use std::path::Path;
use tantivy::{
    collector::TopDocs,
    doc,
    query::QueryParser,
    schema::{Schema, TEXT, STORED, Value},
    Index, IndexWriter, TantivyDocument,
};
use tracing::{debug, info};
use uuid::Uuid;

pub struct KeywordIndex {
    index:  Index,
    schema: Schema,
}

impl KeywordIndex {
    /// Open or create a Tantivy index at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        std::fs::create_dir_all(&path)?;

        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("chunk_id", TEXT | STORED);
        schema_builder.add_text_field("content",  TEXT | STORED);
        let schema = schema_builder.build();

        let index = Index::open_or_create(
            tantivy::directory::MmapDirectory::open(&path)?,
            schema.clone(),
        )?;

        Ok(Self { index, schema })
    }

    /// Add a chunk to the keyword index.
    pub fn index_chunk(&self, chunk_id: Uuid, content: &str) -> Result<()> {
        let mut writer: IndexWriter = self.index.writer(50_000_000)?;

        let chunk_id_field = self.schema.get_field("chunk_id").unwrap();
        let content_field  = self.schema.get_field("content").unwrap();

        writer.add_document(doc!(
            chunk_id_field => chunk_id.to_string(),
            content_field  => content,
        ))?;

        writer.commit()?;
        debug!("Indexed chunk {} in Tantivy", chunk_id);
        Ok(())
    }

    /// Search for a query string, return top_k chunk IDs with scores.
    pub fn search(&self, query_str: &str, top_k: usize) -> Result<Vec<(Uuid, f32)>> {
        let reader = self.index
            .reader_builder()
            .reload_policy(tantivy::ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        let searcher = reader.searcher();
        let content_field  = self.schema.get_field("content").unwrap();
        let chunk_id_field = self.schema.get_field("chunk_id").unwrap();

        let query_parser = QueryParser::for_index(&self.index, vec![content_field]);
        let query = query_parser.parse_query(query_str)?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(top_k))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;
            if let Some(id_val) = doc.get_first(chunk_id_field) {
                if let Some(id_str) = id_val.as_str() {
                    if let Ok(id) = Uuid::parse_str(id_str) {
                        results.push((id, score));
                    }
                }
            }
        }

        info!("Keyword search '{}' → {} results", query_str, results.len());
        Ok(results)
    }
}