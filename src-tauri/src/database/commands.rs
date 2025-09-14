/// Tauri commands for database operations and migration
/// These commands will be exposed to the frontend for testing and management

use tauri::command;
use crate::database::{
    migration_utils::{auto_migrate_if_needed, CacheMigrator, MigrationStatus},
    cache_service::SqliteCacheService,
    DatabaseManager,
};
use anyhow::Result;

#[command]
pub async fn migrate_json_to_sqlite() -> Result<String, String> {
    match auto_migrate_if_needed() {
        Ok(Some(result)) => {
            Ok(format!(
                "Migration completed: {} game names, {} game details migrated",
                result.game_names_migrated, result.game_details_migrated
            ))
        }
        Ok(None) => {
            Ok("No migration needed or already completed".to_string())
        }
        Err(e) => Err(format!("Migration failed: {}", e)),
    }
}

#[command]
pub async fn get_migration_status() -> Result<String, String> {
    let migrator = CacheMigrator::new().map_err(|e| e.to_string())?;
    let status = migrator.get_migration_status().map_err(|e| e.to_string())?;
    Ok(status.to_string())
}

#[command]
pub async fn get_database_stats() -> Result<DatabaseStats, String> {
    let cache_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("zenith-launcher")
        .join("cache");
    
    let db_path = cache_dir.join("games.db");
    let db = DatabaseManager::new(db_path.clone()).map_err(|e| e.to_string())?;
    
    let stats = db.get_stats().map_err(|e| e.to_string())?;
    
    Ok(DatabaseStats {
        games_count: stats.games_count,
        game_details_count: stats.game_details_count,
        library_count: stats.library_count,
        file_size_mb: stats.file_size_bytes as f64 / 1024.0 / 1024.0,
        database_exists: db_path.exists(),
    })
}

#[command]
pub async fn cleanup_expired_cache() -> Result<String, String> {
    let cache_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("zenith-launcher")
        .join("cache");
    
    let db_path = cache_dir.join("games.db");
    let db = DatabaseManager::new(db_path.clone()).map_err(|e| e.to_string())?;
    
    let cleanup_result = db.cleanup_expired().map_err(|e| e.to_string())?;
    
    Ok(format!(
        "Cleanup completed: {} games, {} details removed",
        cleanup_result.games_deleted, cleanup_result.details_deleted
    ))
}

#[command]
pub async fn vacuum_database() -> Result<String, String> {
    let cache_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("zenith-launcher")
        .join("cache");
    
    let db_path = cache_dir.join("games.db");
    let db = DatabaseManager::new(db_path.clone()).map_err(|e| e.to_string())?;
    
    db.vacuum().map_err(|e| e.to_string())?;
    
    Ok("Database vacuum completed".to_string())
}

#[command]
pub async fn test_sqlite_connection() -> Result<String, String> {
    let service = SqliteCacheService::new().map_err(|e| e.to_string())?;
    
    // Try to perform a simple operation
    match service.clear_cache() {
        Ok(_) => Ok("SQLite connection test successful".to_string()),
        Err(e) => Err(format!("SQLite connection test failed: {}", e)),
    }
}

#[command]
pub async fn restore_json_backup() -> Result<String, String> {
    let migrator = CacheMigrator::new().map_err(|e| e.to_string())?;
    migrator.restore_from_backup().map_err(|e| e.to_string())?;
    Ok("JSON backup restored successfully".to_string())
}

/// Database statistics for frontend display
#[derive(serde::Serialize)]
pub struct DatabaseStats {
    pub games_count: i64,
    pub game_details_count: i64,
    pub library_count: i64,
    pub file_size_mb: f64,
    pub database_exists: bool,
}

/// Safely refresh multiple games using batch processing
#[command]
pub async fn batch_refresh_games(app_ids: Vec<String>) -> Result<String, String> {
    use crate::database::legacy_adapter::SQLITE_GAME_CACHE_ADAPTER;
    
    let result = SQLITE_GAME_CACHE_ADAPTER.batch_refresh_games(app_ids).await
        .map_err(|e| e.to_string())?;
    
    Ok(format!(
        "Batch refresh completed: {}/{} successful, {} failed, {} skipped due to circuit breaker",
        result.successfully_processed,
        result.total_requested,
        result.failed,
        result.skipped_circuit_breaker
    ))
}

/// Smart refresh library games based on staleness priority
#[command]
pub async fn smart_refresh_library(app_ids: Vec<String>) -> Result<String, String> {
    use crate::database::legacy_adapter::SQLITE_GAME_CACHE_ADAPTER;
    
    let result = SQLITE_GAME_CACHE_ADAPTER.smart_refresh_library(app_ids).await
        .map_err(|e| e.to_string())?;
    
    Ok(format!(
        "Smart library refresh completed: {}/{} games processed ({} successful, {} failed, {} skipped)",
        result.successfully_processed + result.failed,
        result.total_requested,
        result.successfully_processed,
        result.failed,
        result.skipped_circuit_breaker
    ))
}

/// Get current cache configuration
#[command]
pub async fn get_cache_config() -> Result<CacheConfigInfo, String> {
    use crate::database::legacy_adapter::SQLITE_GAME_CACHE_ADAPTER;
    
    let config = SQLITE_GAME_CACHE_ADAPTER.sqlite_service().get_config();
    
    Ok(CacheConfigInfo {
        max_concurrent_requests: config.max_concurrent_requests,
        batch_size: config.batch_size,
        batch_delay_seconds: config.batch_delay_seconds,
        request_delay_ms: config.request_delay_ms,
        circuit_breaker_threshold: config.circuit_breaker_threshold,
        max_retries: config.max_retries,
    })
}

/// Cache configuration info for frontend
#[derive(serde::Serialize)]
pub struct CacheConfigInfo {
    pub max_concurrent_requests: usize,
    pub batch_size: usize,
    pub batch_delay_seconds: u64,
    pub request_delay_ms: u64,
    pub circuit_breaker_threshold: u32,
    pub max_retries: u32,
}
