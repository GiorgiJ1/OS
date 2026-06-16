pub mod skill;
pub mod registry;
pub mod builtin;

pub use skill::{Skill, SkillOutput};
pub use registry::SkillRegistry;
pub use builtin::{FileReadSkill, FileWriteSkill, WebSearchSkill};

/// Build the default skill registry with all built-in skills.
pub fn default_registry() -> SkillRegistry {
    let mut registry = SkillRegistry::new();
    registry.register(std::sync::Arc::new(FileReadSkill::new()));
    registry.register(std::sync::Arc::new(FileWriteSkill::new()));
    registry.register(std::sync::Arc::new(WebSearchSkill::new()));
    registry
}