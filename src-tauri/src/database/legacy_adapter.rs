/// Legacy adapter to gradually replace the old GameCache with SQLite
/// This allows us to migrate step by step without breaking existing code

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use crate::database::cache_service::SqliteCacheService;
use crate::GameDetail;

/// Adapter that provides the old GameCache interface but uses SQLite underneath
pub struct LegacyGameCacheAdapter {
    sqlite_service: Arc<SqliteCacheService>,
    // Keep some in-memory structures for compatibility during transition
    _legacy_in_flight: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    _cache_dir: PathBuf,
}

impl LegacyGameCacheAdapter {
    pub fn new() -> anyhow::Result<Self> {
        let cache_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zenith-launcher")
            .join("cache");

        Ok(Self {
            sqlite_service: Arc::new(SqliteCacheService::new()?),
            _legacy_in_flight: Arc::new(Mutex::new(HashMap::new())),
            _cache_dir: cache_dir,
        })
    }

    // Methods that match the old GameCache interface

    pub async fn get_game_details(&self, app_id: &str) -> Option<GameDetail> {
        self.sqlite_service.get_game_details(app_id).await
    }

    pub fn set_game_details(&self, app_id: String, details: GameDetail) {
        if let Err(e) = self.sqlite_service.set_game_details(app_id, details) {
            eprintln!("Failed to cache game details: {}", e);
        }
    }

    pub async fn get_game_name(&self, app_id: &str) -> Option<String> {
        self.sqlite_service.get_game_name(app_id).await
    }

    pub fn set_game_name(&self, app_id: String, name: String) {
        if let Err(e) = self.sqlite_service.set_game_name(app_id, name) {
            eprintln!("Failed to cache game name: {}", e);
        }
    }

    pub async fn get_or_create_request_lock(&self, app_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.sqlite_service.get_or_create_request_lock(app_id).await
    }

    pub async fn remove_request_lock(&self, app_id: &str) {
        self.sqlite_service.remove_request_lock(app_id).await
    }

    pub async fn throttle_request(&self) {
        self.sqlite_service.throttle_request().await
    }

    pub async fn record_error(&self) {
        self.sqlite_service.record_error().await
    }

    pub async fn reset_error_count(&self) {
        self.sqlite_service.reset_error_count().await
    }

    pub fn cleanup_expired(&self) {
        if let Err(e) = self.sqlite_service.cleanup_expired() {
            eprintln!("Failed to cleanup expired entries: {}", e);
        }
    }

    pub fn cache_stats(&self) {
        if let Err(e) = self.sqlite_service.cache_stats() {
            eprintln!("Failed to get cache stats: {}", e);
        }
    }

    pub fn save_to_disk(&self) {
        self.sqlite_service.save_to_disk();
    }

    pub fn load_from_disk(&self) {
        self.sqlite_service.load_from_disk();
    }

    pub fn clear_cache(&self) {
        if let Err(e) = self.sqlite_service.clear_cache() {
            eprintln!("Failed to clear cache: {}", e);
        }
    }

    pub fn invalidate_game_details(&self, app_id: &str) {
        if let Err(e) = self.sqlite_service.invalidate_game_details(app_id) {
            eprintln!("Failed to invalidate game details for {}: {}", app_id, e);
        }
    }

    // Additional compatibility methods for circuit breaker
    pub async fn is_circuit_breaker_open(&self) -> bool {
        self.sqlite_service.is_circuit_breaker_open().await
    }

    // Additional methods needed for compatibility with old GameCache interface
    
    pub fn queue_for_refresh(&self, app_id: String) {
        // For SQLite, we don't need explicit queue management as it handles this internally
        println!("Queue refresh for {}", app_id);
    }

    pub async fn process_queue_batch(&self) {
        // SQLite handles background refreshing automatically, so this is a no-op
        println!("SQLite: Queue processing handled automatically");
    }

    pub fn clear_all(&self) {
        // Alias for clear_cache for backward compatibility
        self.clear_cache();
    }

    // Expose circuit breaker field for backward compatibility
    pub async fn circuit_breaker_open(&self) -> std::sync::Arc<std::sync::Mutex<bool>> {
        // Create a compatible field for legacy code
        std::sync::Arc::new(std::sync::Mutex::new(self.is_circuit_breaker_open().await))
    }

    // Expose the underlying SQLite service for advanced operations
    pub fn sqlite_service(&self) -> &SqliteCacheService {
        &self.sqlite_service
    }

    /// Batch refresh multiple games safely with rate limiting
    pub async fn batch_refresh_games(&self, app_ids: Vec<String>) -> anyhow::Result<crate::database::cache_service::BatchRefreshResult> {
        self.sqlite_service.batch_refresh_games(app_ids).await
    }

    /// Smart refresh library games based on staleness priority
    pub async fn smart_refresh_library(&self, library_games: Vec<String>) -> anyhow::Result<crate::database::cache_service::BatchRefreshResult> {
        self.sqlite_service.smart_refresh_library(library_games).await
    }
}

// Global static instance to replace the old GAME_CACHE
lazy_static::lazy_static! {
    pub static ref SQLITE_GAME_CACHE_ADAPTER: LegacyGameCacheAdapter = {
        LegacyGameCacheAdapter::new().expect("Failed to initialize SQLite game cache adapter")
    };
}
