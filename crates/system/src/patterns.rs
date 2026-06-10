use aios_memory::Database;
use anyhow::Result;
use chrono::{Timelike, Utc};
use tracing::info;

pub struct PatternEngine<'a> {
    db: &'a Database,
}

impl<'a> PatternEngine<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Analyse recent activity and store patterns as memories.
    pub fn analyse_and_store(&self) -> Result<()> {
        self.store_work_session_time()?;
        self.store_active_projects()?;
        self.store_most_active_files()?;
        Ok(())
    }

    fn store_work_session_time(&self) -> Result<()> {
        let hour = Utc::now().hour();
        let session = match hour {
            5..=11  => "morning",
            12..=16 => "afternoon",
            17..=21 => "evening",
            _       => "night",
        };
        self.db.set_memory(
            "current_work_session",
            session,
            Some("pattern"),
        )?;
        info!("Pattern: work session = {}", session);
        Ok(())
    }

    fn store_active_projects(&self) -> Result<()> {
        let mut stmt = self.db.conn.prepare(
            "SELECT DISTINCT path FROM file_activity
             WHERE last_seen > datetime('now', '-3 days')
             ORDER BY count DESC LIMIT 20",
        )?;

        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

        let mut projects = std::collections::HashSet::new();
        for r in rows {
            let path = r?;
            let p = std::path::Path::new(&path);
            // Walk up to find a project root (contains Cargo.toml, package.json, etc.)
            let mut current = p.parent();
            while let Some(dir) = current {
                if dir.join("Cargo.toml").exists()
                    || dir.join("package.json").exists()
                    || dir.join(".git").exists()
                {
                    if let Some(name) = dir.file_name() {
                        projects.insert(name.to_string_lossy().to_string());
                    }
                    break;
                }
                current = dir.parent();
            }
        }

        if !projects.is_empty() {
            let project_list = projects.into_iter().collect::<Vec<_>>().join(", ");
            self.db.set_memory("active_projects", &project_list, Some("pattern"))?;
            info!("Pattern: active projects = {}", project_list);
        }

        Ok(())
    }

    fn store_most_active_files(&self) -> Result<()> {
        let mut stmt = self.db.conn.prepare(
            "SELECT path, SUM(count) as total
             FROM file_activity
             WHERE last_seen > datetime('now', '-1 day')
             GROUP BY path
             ORDER BY total DESC
             LIMIT 5",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;

        let mut files = Vec::new();
        for r in rows {
            let (path, _) = r?;
            if let Some(name) = std::path::Path::new(&path).file_name() {
                files.push(name.to_string_lossy().to_string());
            }
        }

        if !files.is_empty() {
            self.db.set_memory(
                "recently_active_files",
                &files.join(", "),
                Some("pattern"),
            )?;
            info!("Pattern: recently active = {}", files.join(", "));
        }

        Ok(())
    }
}