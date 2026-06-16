use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillOutput {
    pub label: String,
    pub content: String,
    pub success: bool,

}

impl SkillOutput{
    pub fn ok(label: impl Into<String>, content: impl Into<String>) -> Self {
        Self { label: label.into(), content: content.into(), success: true}
    }

    pub fn err(label: impl Into<String>, error: impl Into<String>) -> Self{
        Self { label: label.into(), content: error.into(), success: false}
    }
}

// Every skill implements this trait
#[async_trait]
pub trait Skill: Send + Sync {
    // For registry lookup
    fn name(&self) -> &str;

    // One-line description of what this skill does 
    fn description(&self) -> &str;

    // basically assistant reviews which skills to run
    fn can_handle(&self, input: &str) -> bool;

    // execute the skill with the user's input
    async fn execute(&self, input: &str) -> Result<SkillOutput>;
}