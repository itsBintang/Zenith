/// Tauri commands for database operations and migration
/// These commands will be exposed to the frontend for testing and management

use tauri::command;
use crate::database::{
    migration_utils::{auto_migrate_if_needed, CacheMigrator},
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
pub async fn debug_cache_entry(app_id: String) -> Result<String, String> {
    let cache_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("zenith-launcher")
        .join("cache");
    
    let db_path = cache_dir.join("games.db");
    let db = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    let result = db.with_connection(|conn| {
        use crate::database::operations::GameDetailOperations;
        GameDetailOperations::get_by_id(conn, &app_id)
    }).map_err(|e| e.to_string())?;
    
    match result {
        Some(detail) => {
            let now = chrono::Utc::now().timestamp();
            let expired_categories = detail.get_expired_categories();
            
            Ok(format!(
                "Cache Entry for {}: 
- Name: {}
- Cached at: {} ({}s ago)
- Global expires at: {} ({} expired: {})
- Dynamic expires at: {} ({} expired: {})
- SemiStatic expires at: {} ({} expired: {})
- Static expires at: {} ({} expired: {})
- Expired categories: {:?}
- is_expired(): {}
- has_any_expired(): {}",
                app_id,
                detail.name,
                detail.cached_at,
                now - detail.cached_at,
                detail.expires_at,
                if now > detail.expires_at { "EXPIRED" } else { "FRESH" },
                now > detail.expires_at,
                detail.dynamic_expires_at,
                if now > detail.dynamic_expires_at { "EXPIRED" } else { "FRESH" },
                now > detail.dynamic_expires_at,
                detail.semistatic_expires_at,
                if now > detail.semistatic_expires_at { "EXPIRED" } else { "FRESH" },
                now > detail.semistatic_expires_at,
                detail.static_expires_at,
                if now > detail.static_expires_at { "EXPIRED" } else { "FRESH" },
                now > detail.static_expires_at,
                expired_categories,
                detail.is_expired(),
                detail.has_any_expired()
            ))
        }
        None => Ok(format!("No cache entry found for app_id: {}", app_id))
    }
}

#[command]
pub async fn force_clear_cache() -> Result<String, String> {
    let cache_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("zenith-launcher")
        .join("cache");
    
    let db_path = cache_dir.join("games.db");
    let db = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    db.with_connection(|conn| {
        conn.execute("DELETE FROM game_details", [])?;
        conn.execute("DELETE FROM games", [])?;
        Ok(())
    }).map_err(|e: anyhow::Error| e.to_string())?;
    
    Ok("Cache cleared successfully".to_string())
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
    let service = crate::database::cache_service::SqliteCacheService::new().map_err(|e| e.to_string())?;
    
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

// ============= BYPASS GAMES COMMANDS =============

/// Frontend structure for bypass games (matches src/data/bypassGames.json)
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct FrontendBypassGame {
    #[serde(rename = "appId")]
    pub app_id: String,
    pub name: String,
    pub image: String,
    pub bypasses: Vec<FrontendBypassInfo>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct FrontendBypassInfo {
    pub r#type: u8,
    pub url: String,
}

/// Get all bypass games with SQLite caching (1 month TTL)
#[command]
pub async fn get_bypass_games_cached() -> Result<Vec<FrontendBypassGame>, String> {
    match crate::database::cache_service::SQLITE_CACHE_SERVICE.get_bypass_games().await {
        Ok(bypass_games) => {
            // Convert from database models to frontend format
            let frontend_games = bypass_games
                .into_iter()
                .map(|game| FrontendBypassGame {
                    app_id: game.app_id,
                    name: game.name,
                    image: game.image,
                    bypasses: game.bypasses
                        .into_iter()
                        .map(|bypass| FrontendBypassInfo {
                            r#type: bypass.r#type,
                            url: bypass.url,
                        })
                        .collect(),
                })
                .collect();
            
            Ok(frontend_games)
        }
        Err(e) => {
            eprintln!("Error getting bypass games from cache: {}", e);
            Err(format!("Failed to get bypass games: {}", e))
        }
    }
}

/// Force refresh bypass games cache
#[command]
pub async fn refresh_bypass_games_cache() -> Result<String, String> {
    match crate::database::cache_service::SQLITE_CACHE_SERVICE.refresh_bypass_games().await {
        Ok(games) => Ok(format!("Successfully refreshed {} bypass games", games.len())),
        Err(e) => {
            eprintln!("Error refreshing bypass games cache: {}", e);
            Err(format!("Failed to refresh bypass games cache: {}", e))
        }
    }
}

/// Clear bypass games cache specifically
#[command]
pub async fn clear_bypass_games_cache() -> Result<String, String> {
    let cache_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("zenith-launcher")
        .join("cache");
    
    let db_path = cache_dir.join("games.db");
    let db = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    db.with_connection(|conn| {
        crate::database::operations::BypassGameOperations::clear_all(conn)
    }).map_err(|e: anyhow::Error| e.to_string())?;
    
    Ok("Bypass games cache cleared successfully".to_string())
}

/// Get specific bypass game by app_id
#[command]
pub async fn get_bypass_game_by_id(app_id: String) -> Result<Option<FrontendBypassGame>, String> {
    match crate::database::cache_service::SQLITE_CACHE_SERVICE.get_bypass_game(&app_id).await {
        Ok(Some(game)) => {
            let frontend_game = FrontendBypassGame {
                app_id: game.app_id,
                name: game.name,
                image: game.image,
                bypasses: game.bypasses
                    .into_iter()
                    .map(|bypass| FrontendBypassInfo {
                        r#type: bypass.r#type,
                        url: bypass.url,
                    })
                    .collect(),
            };
            Ok(Some(frontend_game))
        }
        Ok(None) => Ok(None),
        Err(e) => {
            eprintln!("Error getting bypass game {}: {}", app_id, e);
            Err(format!("Failed to get bypass game: {}", e))
        }
    }
}

/// Get bypass games cache statistics
#[command]
pub async fn get_bypass_games_cache_stats() -> Result<BypassGamesCacheStats, String> {
    use crate::database::DatabaseManager;
    use std::path::PathBuf;
    
    let cache_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zenith-launcher")
        .join("cache");
    let db_path = cache_dir.join("games.db");
    
    match DatabaseManager::new(db_path) {
        Ok(db) => {
            match db.with_connection(|conn| {
                use crate::database::operations::BypassGameOperations;
                
                let total_games = BypassGameOperations::count(conn)?;
                let expired_games = BypassGameOperations::get_expired(conn)?.len() as u32;
                let valid_games = total_games - expired_games;
                
                Ok(BypassGamesCacheStats {
                    total_games,
                    valid_games,
                    expired_games,
                    cache_hit_rate: if total_games > 0 {
                        (valid_games as f64 / total_games as f64) * 100.0
                    } else {
                        0.0
                    },
                })
            }) {
                Ok(stats) => Ok(stats),
                Err(e) => Err(format!("Database error: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to connect to database: {}", e)),
    }
}

#[derive(serde::Serialize, Debug)]
pub struct BypassGamesCacheStats {
    pub total_games: u32,
    pub valid_games: u32,
    pub expired_games: u32,
    pub cache_hit_rate: f64,
}
