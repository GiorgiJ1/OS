use crate::skill::{Skill, SkillOutput};
use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, info};

pub struct SkillRegistry {
    skills: Vec<Arc<dyn Skill>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self { skills: Vec::new() }
    }

    pub fn register(&mut self, skill: Arc<dyn Skill>) {
        info!("Registered skill: {}", skill.name());
        self.skills.push(skill);
    }

    /// Find all skills that can handle this input.
    pub fn matching_skills(&self, input: &str) -> Vec<Arc<dyn Skill>> {
        self.skills
            .iter()
            .filter(|s| s.can_handle(input))
            .cloned()
            .collect()
    }

    /// Run all matching skills and return their outputs.
    pub async fn execute_matching(&self, input: &str) -> Vec<SkillOutput> {
        let matching = self.matching_skills(input);

        if matching.is_empty() {
            debug!("No skills matched for input: {}", &input[..input.len().min(50)]);
            return vec![];
        }

        let mut results = Vec::new();
        for skill in matching {
            info!("Running skill: {}", skill.name());
            match skill.execute(input).await {
                Ok(output) => {
                    info!("Skill {} completed: {}", skill.name(), output.label);
                    results.push(output);
                }
                Err(e) => {
                    results.push(SkillOutput::err(
                        skill.name(),
                        format!("Skill failed: {}", e),
                    ));
                }
            }
        }

        results
    }

    /// Format all skill outputs into a context block for the prompt.
    pub fn format_outputs(outputs: &[SkillOutput]) -> String {
        if outputs.is_empty() {
            return String::new();
        }

        let mut ctx = String::from("Results from tools:\n\n");
        for output in outputs {
            if output.success {
                ctx.push_str(&format!("[{}]\n{}\n\n", output.label, output.content));
            } else {
                ctx.push_str(&format!("[{} — failed]\n{}\n\n", output.label, output.content));
            }
        }
        ctx
    }

    pub fn list(&self) -> Vec<(&str, &str)> {
        self.skills.iter().map(|s| (s.name(), s.description())).collect()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}