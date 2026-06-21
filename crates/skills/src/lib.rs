pub mod skill;
pub mod registry;
pub mod builtin;

pub use skill::{Skill, SkillOutput};
pub use registry::SkillRegistry;
pub use builtin::{FileReadSkill, FileWriteSkill, WebSearchSkill, ScreenVisionSkill};

pub fn default_registry() -> SkillRegistry {
    let mut registry = SkillRegistry::new();
    registry.register(std::sync::Arc::new(FileReadSkill::new()));
    registry.register(std::sync::Arc::new(FileWriteSkill::new()));
    registry.register(std::sync::Arc::new(WebSearchSkill::new()));
    registry.register(std::sync::Arc::new(ScreenVisionSkill::new()));
    registry
}