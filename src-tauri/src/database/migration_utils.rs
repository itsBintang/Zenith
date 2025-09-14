/// Utility functions to migrate data from JSON cache to SQLite database
/// This handles the transition from the old cache system to the new one

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use serde::{Deserialize, Serialize};
use crate::database::{DatabaseManager, operations::*};
use crate::database::models::{Game, GameDetailDb};
use crate::GameDetail;

/// Legacy CacheEntry structure for parsing old JSON files
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry<T> {
    data: T,
    timestamp: u64,
    expires_at: u64,
}

/// Migrator for converting JSON cache to SQLite
pub struct CacheMigrator {
    db: DatabaseManager,
    cache_dir: PathBuf,
}

impl CacheMigrator {
    /// Create new migrator
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zenith-launcher")
            .join("cache");

        let db_path = cache_dir.join("games.db");
        let db = DatabaseManager::new(db_path)?;

        Ok(Self {
            db,
            cache_dir,
        })
    }

    /// Run full migration from JSON to SQLite
    pub fn migrate_all(&self) -> Result<MigrationResult> {
        println!("Starting migration from JSON cache to SQLite...");
        
        let mut result = MigrationResult::default();
        
        // Migrate game names
        if let Ok(names_migrated) = self.migrate_game_names() {
            result.game_names_migrated = names_migrated;
            println!("Migrated {} game names", names_migrated);
        }
        
        // Migrate game details
        if let Ok(details_migrated) = self.migrate_game_details() {
            result.game_details_migrated = details_migrated;
            println!("Migrated {} game details", details_migrated);
        }
        
        // Clean up old JSON files after successful migration
        if result.game_names_migrated > 0 || result.game_details_migrated > 0 {
            self.backup_and_cleanup_json_files()?;
            result.json_files_cleaned = true;
        }
        
        println!("Migration completed successfully: {} names, {} details", 
                 result.game_names_migrated, result.game_details_migrated);
        
        Ok(result)
    }

    /// Migrate game names from JSON cache
    fn migrate_game_names(&self) -> Result<usize> {
        let json_path = self.cache_dir.join("game_names.json");
        
        if !json_path.exists() {
            return Ok(0);
        }
        
        let json_content = fs::read_to_string(&json_path)?;
        let game_names_cache: HashMap<String, CacheEntry<String>> = 
            serde_json::from_str(&json_content)?;
        
        let mut migrated_count = 0;
        
        for (app_id, cache_entry) in game_names_cache {
            // Convert to Game struct
            let game = Game::new(
                app_id.clone(),
                cache_entry.data,
                format!("https://cdn.akamai.steamstatic.com/steam/apps/{}/header.jpg", app_id),
                604800, // 7 days TTL
            );
            
            // Insert into SQLite
            self.db.with_connection(|conn| {
                GameOperations::upsert(conn, &game)
            })?;
            
            migrated_count += 1;
        }
        
        Ok(migrated_count)
    }

    /// Migrate game details from JSON cache
    fn migrate_game_details(&self) -> Result<usize> {
        let json_path = self.cache_dir.join("game_details.json");
        
        if !json_path.exists() {
            return Ok(0);
        }
        
        let json_content = fs::read_to_string(&json_path)?;
        let game_details_cache: HashMap<String, CacheEntry<GameDetail>> = 
            serde_json::from_str(&json_content)?;
        
        let mut migrated_count = 0;
        
        for (_app_id, cache_entry) in game_details_cache {
            // Convert to GameDetailDb
            let db_detail: GameDetailDb = cache_entry.data.into();
            
            // Insert into SQLite
            self.db.with_connection(|conn| {
                GameDetailOperations::upsert(conn, &db_detail)
            })?;
            
            migrated_count += 1;
        }
        
        Ok(migrated_count)
    }

    /// Backup and clean up old JSON files
    fn backup_and_cleanup_json_files(&self) -> Result<()> {
        let backup_dir = self.cache_dir.join("json_backup");
        fs::create_dir_all(&backup_dir)?;
        
        // Backup files
        let files_to_backup = ["game_names.json", "game_details.json"];
        
        for filename in &files_to_backup {
            let src_path = self.cache_dir.join(filename);
            let backup_path = backup_dir.join(format!("{}.backup", filename));
            
            if src_path.exists() {
                fs::copy(&src_path, &backup_path)?;
                println!("Backed up {} to {}", filename, backup_path.display());
                
                // Remove original file
                fs::remove_file(&src_path)?;
                println!("Removed original {}", filename);
            }
        }
        
        Ok(())
    }

    /// Check if migration is needed
    pub fn needs_migration(&self) -> bool {
        let game_names_exists = self.cache_dir.join("game_names.json").exists();
        let game_details_exists = self.cache_dir.join("game_details.json").exists();
        
        game_names_exists || game_details_exists
    }

    /// Get migration status
    pub fn get_migration_status(&self) -> Result<MigrationStatus> {
        let json_files_exist = self.needs_migration();
        
        let sqlite_stats = self.db.get_stats()?;
        let has_sqlite_data = sqlite_stats.games_count > 0 || sqlite_stats.game_details_count > 0;
        
        let status = if json_files_exist && !has_sqlite_data {
            MigrationStatus::NotMigrated
        } else if json_files_exist && has_sqlite_data {
            MigrationStatus::PartiallyMigrated
        } else if !json_files_exist && has_sqlite_data {
            MigrationStatus::FullyMigrated
        } else {
            MigrationStatus::NoData
        };
        
        Ok(status)
    }

    /// Restore from JSON backup if needed
    pub fn restore_from_backup(&self) -> Result<()> {
        let backup_dir = self.cache_dir.join("json_backup");
        
        if !backup_dir.exists() {
            return Err(anyhow::anyhow!("No backup directory found"));
        }
        
        let files_to_restore = ["game_names.json.backup", "game_details.json.backup"];
        
        for filename in &files_to_restore {
            let backup_path = backup_dir.join(filename);
            let restore_path = self.cache_dir.join(filename.trim_end_matches(".backup"));
            
            if backup_path.exists() {
                fs::copy(&backup_path, &restore_path)?;
                println!("Restored {} from backup", filename);
            }
        }
        
        Ok(())
    }
}

/// Migration result
#[derive(Debug, Default)]
pub struct MigrationResult {
    pub game_names_migrated: usize,
    pub game_details_migrated: usize,
    pub json_files_cleaned: bool,
}

/// Migration status
#[derive(Debug, PartialEq)]
pub enum MigrationStatus {
    NotMigrated,      // JSON files exist, no SQLite data
    PartiallyMigrated, // Both JSON and SQLite data exist
    FullyMigrated,    // Only SQLite data exists
    NoData,           // No data in either format
}

impl std::fmt::Display for MigrationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationStatus::NotMigrated => write!(f, "Not migrated - JSON cache found"),
            MigrationStatus::PartiallyMigrated => write!(f, "Partially migrated - both JSON and SQLite data found"),
            MigrationStatus::FullyMigrated => write!(f, "Fully migrated - using SQLite cache"),
            MigrationStatus::NoData => write!(f, "No cache data found"),
        }
    }
}

/// Auto-migration function that can be called at startup
pub fn auto_migrate_if_needed() -> Result<Option<MigrationResult>> {
    let migrator = CacheMigrator::new()?;
    
    match migrator.get_migration_status()? {
        MigrationStatus::NotMigrated => {
            println!("JSON cache detected, starting automatic migration...");
            let result = migrator.migrate_all()?;
            Ok(Some(result))
        }
        MigrationStatus::PartiallyMigrated => {
            println!("Warning: Both JSON and SQLite cache found. Manual intervention may be needed.");
            Ok(None)
        }
        MigrationStatus::FullyMigrated => {
            println!("Using SQLite cache (migration already completed)");
            Ok(None)
        }
        MigrationStatus::NoData => {
            println!("No existing cache data found, starting fresh with SQLite");
            Ok(None)
        }
    }
}
