use aios_memory::Database;
use anyhow::Result;
// no import needed
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SemanticResult {
    pub chunk_id:  Uuid,
    pub score:     f32,
}

/// Dot product of two vectors — A · B
fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Magnitude of a vector — |A|
fn magnitude(a: &[f32]) -> f32 {
    a.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Cosine similarity — (A · B) / (|A| × |B|)
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mag_a = magnitude(a);
    let mag_b = magnitude(b);
    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    dot(a, b) / (mag_a * mag_b)
}

/// Search all stored embeddings for the closest matches to query_vector.
/// Returns top_k results sorted by score descending.
pub fn semantic_search(
    db:           &Database,
    query_vector: &[f32],
    model:        &str,
    top_k:        usize,
) -> Result<Vec<SemanticResult>> {
    let all = db.get_all_embeddings(model)?;

    let mut scored: Vec<SemanticResult> = all
        .iter()
        .map(|(chunk_id, vector)| SemanticResult {
            chunk_id: *chunk_id,
            score:    cosine_similarity(query_vector, vector),
        })
        .collect();

    // Sort descending by score
    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    scored.truncate(top_k);

    Ok(scored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_vectors() {
        let a = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b)).abs() < 1e-6);
    }

    #[test]
    fn test_known_similarity() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.9746).abs() < 0.001);
    }
}