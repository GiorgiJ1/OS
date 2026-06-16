use crate::skill::{Skill, SkillOutput};
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use std::path::Path;
use tracing::debug;

pub struct FileReadSkill;

impl FileReadSkill {
    pub fn new() -> Self { Self }

    fn extract_path(input: &str) -> Option<String> {
        // Match Windows and Unix paths
        let patterns = [
            r#"[A-Za-z]:\\[^\s"']+|/[^\s"']+"#,
            r#"["']([^"']+\.[a-zA-Z]{1,5})["']"#,
            r#"file\s+(?:called|named)\s+([^\s,]+)"#,
            r#"read\s+([^\s,]+\.[a-zA-Z]{1,5})"#,
            r#"open\s+([^\s,]+\.[a-zA-Z]{1,5})"#,
        ];

        for pattern in &patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(cap) = re.captures(input) {
                    let path = cap.get(1)
                        .or_else(|| cap.get(0))
                        .map(|m| m.as_str().trim_matches(|c| c == '"' || c == '\''))
                        .map(String::from);
                    if let Some(p) = path {
                        if Path::new(&p).exists() {
                            return Some(p);
                        }
                    }
                }
            }
        }
        None
    }
}

#[async_trait]
impl Skill for FileReadSkill {
    fn name(&self) -> &str { "file_read" }

    fn description(&self) -> &str {
        "Read the contents of a file from disk"
    }

    fn can_handle(&self, input: &str) -> bool {
        let lower = input.to_lowercase();
        let has_trigger = lower.contains("read")
            || lower.contains("open")
            || lower.contains("show me")
            || lower.contains("what's in")
            || lower.contains("whats in")
            || lower.contains("contents of")
            || lower.contains("inside");

        let has_path = input.contains('\\')
            || input.contains('/')
            || input.contains(".txt")
            || input.contains(".md")
            || input.contains(".pdf")
            || input.contains(".rs")
            || input.contains(".py")
            || input.contains(".json")
            || input.contains(".toml");

        has_trigger && has_path
    }

    async fn execute(&self, input: &str) -> Result<SkillOutput> {
        let path = match Self::extract_path(input) {
            Some(p) => p,
            None    => return Ok(SkillOutput::err(
                "file_read",
                "Could not find a valid file path in the request",
            )),
        };

        debug!("Reading file: {}", path);

        let content = tokio::fs::read_to_string(&path).await?;
        let preview = if content.len() > 4000 {
            format!("{}\n\n[... truncated, {} chars total]", &content[..4000], content.len())
        } else {
            content
        };

        Ok(SkillOutput::ok(
            format!("file: {}", path),
            preview,
        ))
    }
}