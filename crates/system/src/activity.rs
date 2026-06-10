use aios_memory::Database;
use anyhow::Result;
use chrono::Utc;
use rusqlite::params;
use std::path::Path;
use tracing::debug;

/// Log a file access event to the DB.
pub struct ActivityTracker<'a> {
    db: &'a Database,
}

impl<'a> ActivityTracker<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn log_file_event(&self, path: &Path, event_type: &str) -> Result<()> {
        let path_str = path.to_string_lossy().to_string();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown")
            .to_string();
        let now = Utc::now().to_rfc3339();

        self.db.conn.execute(
            "INSERT OR IGNORE INTO file_activity (path, extension, event_type, last_seen, count)
             VALUES (?1, ?2, ?3, ?4, 0)",
            params![path_str, ext, event_type, now],
        )?;

        self.db.conn.execute(
            "UPDATE file_activity SET count = count + 1, last_seen = ?1
             WHERE path = ?2 AND event_type = ?3",
            params![now, path_str, event_type],
        )?;

        debug!("Activity logged: {} — {}", event_type, path_str);
        Ok(())
    }

    /// Get the most actively changed files in the last N days.
    pub fn get_active_files(&self, days: i64, limit: usize) -> Result<Vec<(String, i64)>> {
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let mut stmt = self.db.conn.prepare(
            "SELECT path, SUM(count) as total
             FROM file_activity
             WHERE last_seen > ?1
             GROUP BY path
             ORDER BY total DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![cutoff, limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        rows.map(|r| Ok(r?)).collect()
    }

    /// Get active projects based on directory activity.
    pub fn get_active_projects(&self, days: i64) -> Result<Vec<String>> {
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let mut stmt = self.db.conn.prepare(
            "SELECT DISTINCT path FROM file_activity WHERE last_seen > ?1",
        )?;
        let rows = stmt.query_map(params![cutoff], |row| {
            row.get::<_, String>(0)
        })?;

        let mut dirs = std::collections::HashSet::new();
        for r in rows {
            let path = r?;
            if let Some(parent) = std::path::Path::new(&path).parent() {
                if let Some(name) = parent.file_name() {
                    dirs.insert(name.to_string_lossy().to_string());
                }
            }
        }

        Ok(dirs.into_iter().collect())
    }
}