use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;
use tokio::sync::mpsc as tokio_mpsc;
use tracing::{debug, info, warn};

/// File extensions AIOS cares about
const WATCHED_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "txt", "md", "rs", "py", "js", "ts",
    "json", "yaml", "yml", "toml", "html", "csv", "org",
];

/// Paths to always ignore
const IGNORED_DIRS: &[&str] = &[
    "target", "node_modules", ".git", ".cache",
    "AppData", "Windows", "Program Files", "Program Files (x86)",
    "$Recycle.Bin", "System Volume Information",
];

#[derive(Debug, Clone)]
pub enum FileEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
}

pub struct FilesystemWatcher {
    watched_roots: Vec<PathBuf>,
}

impl FilesystemWatcher {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self { watched_roots: roots }
    }

    pub fn with_default_drives() -> Self {
        Self::new(vec![
            PathBuf::from("D:\\"),
            PathBuf::from("G:\\"),
        ])
    }

    /// Start watching. Returns a receiver that emits FileEvents.
    pub fn start(&self) -> Result<tokio_mpsc::Receiver<FileEvent>> {
        let (tx, rx) = tokio_mpsc::channel::<FileEvent>(1024);
        let roots = self.watched_roots.clone();

        // notify uses std channels internally so we bridge to tokio
        let (std_tx, std_rx) = std_mpsc::channel::<notify::Result<Event>>();

        let mut watcher = RecommendedWatcher::new(std_tx, notify::Config::default()
            .with_poll_interval(Duration::from_secs(2)))?;

        for root in &roots {
            if root.exists() {
                watcher.watch(root, RecursiveMode::Recursive)?;
                info!("Watching: {}", root.display());
            } else {
                warn!("Drive not found, skipping: {}", root.display());
            }
        }

        // Spawn a thread to bridge std channel → tokio channel
        tokio::task::spawn_blocking(move || {
            // Keep watcher alive in this thread
            let _watcher = watcher;

            for result in std_rx {
                match result {
                    Ok(event) => {
                        for path in &event.paths {
                            if !should_watch(path) {
                                continue;
                            }
                            let file_event = match event.kind {
                                EventKind::Create(_) => FileEvent::Created(path.clone()),
                                EventKind::Modify(_) => FileEvent::Modified(path.clone()),
                                EventKind::Remove(_) => FileEvent::Deleted(path.clone()),
                                _ => continue,
                            };
                            debug!("File event: {:?}", file_event);
                            let _ = tx.blocking_send(file_event);
                        }
                    }
                    Err(e) => warn!("Watch error: {}", e),
                }
            }
        });

        Ok(rx)
    }
}

/// Returns true if this path should be watched and indexed.
pub fn should_watch(path: &Path) -> bool {
    // Check ignored directories
    for component in path.components() {
        let s = component.as_os_str().to_string_lossy();
        if IGNORED_DIRS.iter().any(|ig| s.eq_ignore_ascii_case(ig)) {
            return false;
        }
    }

    // Check file extension
    if let Some(ext) = path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        return WATCHED_EXTENSIONS.contains(&ext.as_str());
    }

    false
}