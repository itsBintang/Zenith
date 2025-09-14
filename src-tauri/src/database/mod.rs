use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub mod models;
pub mod migrations;
pub mod operations;
pub mod cache_service;
pub mod legacy_adapter;
pub mod migration_utils;
pub mod commands;
pub mod ttl_config;

/// Database manager for SQLite operations
pub struct DatabaseManager {
    connection: Arc<Mutex<Connection>>,
    db_path: PathBuf,
}

impl DatabaseManager {
    /// Create a new database manager instance
    pub fn new(db_path: PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Open database with appropriate flags
        let conn = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE 
                | OpenFlags::SQLITE_OPEN_CREATE 
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        // Configure SQLite for better performance
        conn.execute_batch("
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA cache_size = 1000;
            PRAGMA foreign_keys = ON;
            PRAGMA temp_store = MEMORY;
        ")?;

        let manager = Self {
            connection: Arc::new(Mutex::new(conn)),
            db_path,
        };

        // Run migrations
        manager.run_migrations()?;

        Ok(manager)
    }

    /// Get database file path
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Execute a function with database connection
    pub fn with_connection<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.connection.lock().unwrap();
        f(&*conn)
    }

    /// Run database migrations
    fn run_migrations(&self) -> Result<()> {
        self.with_connection(|conn| {
            migrations::run_migrations(conn)
        })
    }

    /// Get database statistics
    pub fn get_stats(&self) -> Result<DatabaseStats> {
        self.with_connection(|conn| {
            let games_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM games", 
                [], 
                |row| row.get(0)
            )?;
            
            let game_details_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM game_details", 
                [], 
                |row| row.get(0)
            )?;
            
            let library_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM user_library", 
                [], 
                |row| row.get(0)
            )?;

            // Get database file size
            let file_size = std::fs::metadata(&self.db_path)
                .map(|m| m.len())
                .unwrap_or(0);

            Ok(DatabaseStats {
                games_count,
                game_details_count,
                library_count,
                file_size_bytes: file_size,
            })
        })
    }

    /// Clean up expired cache entries
    pub fn cleanup_expired(&self) -> Result<CleanupResult> {
        self.with_connection(|conn| {
            let now = chrono::Utc::now().timestamp();
            
            let games_deleted: usize = conn.execute(
                "DELETE FROM games WHERE expires_at < ?1",
                [now],
            )?;
            
            let details_deleted: usize = conn.execute(
                "DELETE FROM game_details WHERE expires_at < ?1",
                [now],
            )?;

            // Update last cleanup time
            conn.execute(
                "INSERT OR REPLACE INTO cache_metadata (key, value, updated_at) 
                 VALUES ('last_cleanup', ?1, ?1)",
                [now],
            )?;

            Ok(CleanupResult {
                games_deleted,
                details_deleted,
            })
        })
    }

    /// Vacuum database to reclaim space
    pub fn vacuum(&self) -> Result<()> {
        self.with_connection(|conn| {
            conn.execute("VACUUM", [])?;
            Ok(())
        })
    }

    /// Close database connection
    pub fn close(self) -> Result<()> {
        // The Arc<Mutex<Connection>> will be dropped and connection closed
        Ok(())
    }
}

/// Database statistics
#[derive(Debug)]
pub struct DatabaseStats {
    pub games_count: i64,
    pub game_details_count: i64,
    pub library_count: i64,
    pub file_size_bytes: u64,
}

/// Cleanup operation result
#[derive(Debug)]
pub struct CleanupResult {
    pub games_deleted: usize,
    pub details_deleted: usize,
}

impl std::fmt::Display for DatabaseStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Database Stats: {} games, {} details, {} library entries, {:.2} MB",
            self.games_count,
            self.game_details_count,
            self.library_count,
            self.file_size_bytes as f64 / 1024.0 / 1024.0
        )
    }
}
