pub mod watcher;
pub mod activity;
pub mod patterns;
pub mod vision;
pub mod ocr;

pub use watcher::FilesystemWatcher;
pub use activity::ActivityTracker;
pub use patterns::PatternEngine;
pub use vision::{capture_screen, capture_screen_with_target, list_screens, CaptureTarget, CaptureResult, ScreenInfo};
pub use ocr::extract_text;