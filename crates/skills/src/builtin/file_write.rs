use crate::skill::{Skill, SkillOutput};
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use tracing::info;

pub struct FileWriteSkill;

impl FileWriteSkill {
    pub fn new() -> Self { Self }
}

#[async_trait]
impl Skill for FileWriteSkill {
    fn name(&self) -> &str { "file_write" }

    fn description(&self) -> &str {
        "Write or save content to a file on disk"
    }

    fn can_handle(&self, input: &str) -> bool {
        let lower = input.to_lowercase();
        (lower.contains("save") || lower.contains("write") || lower.contains("create"))
            && (input.contains(":\\") || input.contains("./") || input.contains(".txt")
                || input.contains(".md") || input.contains(".json") || input.contains(".rs"))
    }

    async fn execute(&self, input: &str) -> Result<SkillOutput> {
        // Find a Windows or Unix path in the input
        let path = extract_path(input);
        let content = extract_content(input);

        let path = match path {
            Some(p) => p,
            None    => return Ok(SkillOutput::err(
                "file_write",
                "Could not find a file path in the request. Please specify a full path like D:\\file.txt",
            )),
        };

        // Create parent dirs
        if let Some(parent) = Path::new(&path).parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        tokio::fs::write(&path, &content).await?;
        info!("Wrote {} bytes to {}", content.len(), path);

        Ok(SkillOutput::ok(
            format!("saved: {}", path),
            format!("Successfully wrote {} bytes to {}", content.len(), path),
        ))
    }
}

fn extract_path(input: &str) -> Option<String> {
    // Match D:\something.txt or D:/something.txt
    let words: Vec<&str> = input.split_whitespace().collect();
    for word in &words {
        let w = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '\\' && c != '/' && c != ':' && c != '.');
        if (w.len() > 3 && w.contains(":\\")) || w.starts_with('/') {
            if w.contains('.') {
                return Some(w.to_string());
            }
        }
    }
    // Look for quoted paths
    if let Some(start) = input.find('"') {
        if let Some(end) = input[start+1..].find('"') {
            let candidate = &input[start+1..start+1+end];
            if candidate.contains('.') || candidate.contains('\\') {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn extract_content(input: &str) -> String {
    // Look for content after common markers
    let markers = [
        "content to write:",
        "containing:",
        "with content:",
        "content:",
        "with text:",
        "text:",
        ": \"",
        ": '",
    ];
    let lower = input.to_lowercase();
    for marker in &markers {
        if let Some(idx) = lower.find(marker) {
            let content = input[idx + marker.len()..].trim();
            return content.trim_matches(|c| c == '"' || c == '\'').to_string();
        }
    }
    String::new()
}