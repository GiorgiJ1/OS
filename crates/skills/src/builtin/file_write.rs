use crate::skill::{Skill, SkillOutput};
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use std::path::Path;
use tracing::info;

pub struct FileWriteSkill;

impl FileWriteSkill {
    pub fn new() -> Self { Self }

    fn extract_path_and_content(input: &str) -> Option<(String, String)> {
        let patterns = [
            r#"save\s+(?:to\s+)?["']?([^\s"']+\.[a-zA-Z]{1,5})["']?"#,
            r#"write\s+(?:to\s+)?["']?([^\s"']+\.[a-zA-Z]{1,5})["']?"#,
            r#"create\s+(?:a\s+)?(?:file\s+)?["']?([^\s"']+\.[a-zA-Z]{1,5})["']?"#,
        ];

        for pattern in &patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(cap) = re.captures(input) {
                    if let Some(path) = cap.get(1).map(|m| m.as_str().to_string()) {
                        // Extract content after "containing", "with content", ":"
                        let content = extract_content_from_input(input);
                        return Some((path, content));
                    }
                }
            }
        }
        None
    }
}

fn extract_content_from_input(input: &str) -> String {
    let markers = ["containing:", "with content:", "content:", ":\n", " with "];
    for marker in &markers {
        if let Some(idx) = input.find(marker) {
            return input[idx + marker.len()..].trim().to_string();
        }
    }
    // Fall back to everything after the path
    input.to_string()
}

#[async_trait]
impl Skill for FileWriteSkill {
    fn name(&self) -> &str { "file_write" }

    fn description(&self) -> &str {
        "Write or save content to a file on disk"
    }

    fn can_handle(&self, input: &str) -> bool {
        let lower = input.to_lowercase();
        (lower.contains("save") || lower.contains("write") || lower.contains("create file"))
            && (input.contains('.') || input.contains('\\') || input.contains('/'))
    }

    async fn execute(&self, input: &str) -> Result<SkillOutput> {
        let (path, content) = match Self::extract_path_and_content(input) {
            Some(pc) => pc,
            None     => return Ok(SkillOutput::err(
                "file_write",
                "Could not determine file path or content",
            )),
        };

        // Create parent directories if needed
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