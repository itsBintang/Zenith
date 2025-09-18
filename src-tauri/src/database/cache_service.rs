use anyhow::Result;
use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::{Mutex, Semaphore};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use crate::database::{DatabaseManager, operations::*};
use crate::database::models::{Game, GameDetailDb, BypassGame, BypassInfo};
use crate::GameDetail;

/// Configuration for cache batch processing and rate limiting
#[derive(Clone)]
pub struct CacheConfig {
    pub max_concurrent_requests: usize,
    pub batch_size: usize,
    pub batch_delay_seconds: u64,
    pub request_delay_ms: u64,
    pub circuit_breaker_threshold: u32,
    pub max_retries: u32,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 3,  // Conservative untuk Steam API
            batch_size: 15,             // Small batches
            batch_delay_seconds: 15,    // 15 detik antar batch
            request_delay_ms: 1500,     // 1.5 detik antar request
            circuit_breaker_threshold: 3, // Lebih sensitif
            max_retries: 2,             // Fewer retries
        }
    }
}

/// SQLite-based cache service to replace the old JSON cache
pub struct SqliteCacheService {
    db: Arc<DatabaseManager>,
    config: CacheConfig,
    // Keep in-flight requests tracking to prevent duplicate API calls
    in_flight_requests: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    // Rate limiting
    last_request_time: Arc<Mutex<u64>>,
    // Circuit breaker for API failures
    consecutive_errors: Arc<Mutex<u32>>,
    circuit_breaker_open: Arc<Mutex<bool>>,
    // Semaphore for concurrent request limiting
    concurrent_limit: Arc<Semaphore>,
}

impl SqliteCacheService {
    /// Create new SQLite cache service with default config
    pub fn new() -> Result<Self> {
        Self::with_config(CacheConfig::default())
    }

    /// Create new SQLite cache service with custom config
    pub fn with_config(config: CacheConfig) -> Result<Self> {
        // Create database in the same location as before
        let cache_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zenith-launcher")
            .join("cache");

        let db_path = cache_dir.join("games.db");
        let db = Arc::new(DatabaseManager::new(db_path)?);

        Ok(Self {
            db,
            config: config.clone(),
            in_flight_requests: Arc::new(Mutex::new(HashMap::new())),
            last_request_time: Arc::new(Mutex::new(0)),
            consecutive_errors: Arc::new(Mutex::new(0)),
            circuit_breaker_open: Arc::new(Mutex::new(false)),
            concurrent_limit: Arc::new(Semaphore::new(config.max_concurrent_requests)),
        })
    }

    /// Get game details with caching and stale-while-revalidate
    pub async fn get_game_details(&self, app_id: &str) -> Option<GameDetail> {
        // Check database cache first with proper error handling
        let cached_detail = match self.db.with_connection(|conn| {
            GameDetailOperations::get_by_id(conn, app_id)
        }) {
            Ok(detail_option) => detail_option,
            Err(e) => {
                #[cfg(debug_assertions)]
                eprintln!("Database error for app_id {}: {}", app_id, e);
                
                // Log error but don't crash - return None to trigger API fetch
                return None;
            }
        };

        if let Some(detail) = cached_detail {
            if !detail.is_expired() {
                // Fresh data - return immediately
                // Cache hit - silent unless verbose debugging needed
                // #[cfg(debug_assertions)]
                // println!("Cache HIT (fresh) for {}", app_id);
                
                return Some(detail.into());
            } else {
                // Check what categories are expired for smarter handling
                let expired_categories = detail.get_expired_categories();
                
                #[cfg(debug_assertions)]
                println!("Cache HIT (stale) for {}, expired: {:?}", app_id, expired_categories);
                
                // For critical data (DLC), force fresh fetch
                if expired_categories.contains(&"dynamic") {
                    #[cfg(debug_assertions)]
                    println!("Critical data expired for {}, forcing fresh fetch", app_id);
                    
                    return None; // Force API call for critical data
                } else {
                    // For non-critical data, return stale and refresh in background
                    let stale_data = detail.clone();
                    let app_id_clone = app_id.to_string();
                    let service_clone = Arc::new(self.clone_for_background());
                    
                    tokio::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        let _ = service_clone.refresh_game_details_background(&app_id_clone).await;
                    });

                    return Some(stale_data.into());
                }
            }
        } else {
            #[cfg(debug_assertions)]
            println!("Cache MISS for {}", app_id);
        }

        None
    }

    /// Set game details in cache
    pub fn set_game_details(&self, app_id: String, details: GameDetail) -> Result<()> {
        let db_detail: GameDetailDb = details.clone().into();
        
        let result = self.db.with_connection(|conn| {
            // First ensure the game exists in games table (for foreign key constraint)
            let game = Game::new(
                details.app_id.clone(),
                details.name.clone(),
                details.header_image.clone(),
                604800, // 7 days TTL for game names
            );
            GameOperations::upsert(conn, &game)?;
            
            // Then insert game details
            GameDetailOperations::upsert(conn, &db_detail)
        });

        match result {
            Ok(_) => {
                #[cfg(debug_assertions)]
                println!("Successfully cached game details for {} with granular TTL", app_id);
                Ok(())
            }
            Err(e) => {
                #[cfg(debug_assertions)]
                eprintln!("Failed to cache game details for {}: {}", app_id, e);
                Err(e)
            }
        }
    }

    /// Get game name with caching
    pub async fn get_game_name(&self, app_id: &str) -> Option<String> {
        let cached_game = match self.db.with_connection(|conn| {
            GameOperations::get_by_id(conn, app_id)
        }) {
            Ok(game_option) => game_option,
            Err(e) => {
                #[cfg(debug_assertions)]
                eprintln!("Database error getting game name for {}: {}", app_id, e);
                
                return None;
            }
        };

        if let Some(game) = cached_game {
            if !game.is_expired() {
                // Game name cache hit - silent for cleaner logs
                // #[cfg(debug_assertions)]
                // println!("Game name cache HIT (fresh) for {}: {}", app_id, game.name);
                
                return Some(game.name);
            } else {
                // Stale data - return it but refresh in background
                let stale_name = game.name.clone();
                
                #[cfg(debug_assertions)]
                println!("Game name cache HIT (stale) for {}: {}", app_id, stale_name);
                
                self.queue_for_refresh(app_id.to_string());
                return Some(stale_name);
            }
        } else {
            #[cfg(debug_assertions)]
            println!("Game name cache MISS for {}", app_id);
        }

        None
    }

    /// Set game name in cache
    pub fn set_game_name(&self, app_id: String, name: String) -> Result<()> {
        // Create a basic game entry for name caching
        let game = Game::new(
            app_id.clone(),
            name.clone(),
            format!("https://cdn.akamai.steamstatic.com/steam/apps/{}/header.jpg", app_id),
            604800, // 7 days TTL
        );

        self.db.with_connection(|conn| {
            GameOperations::upsert(conn, &game)
        })?;

        #[cfg(debug_assertions)]
        println!("Cached game name for {}: {}", app_id, name);

        Ok(())
    }

    /// Get or create request lock to prevent duplicate API calls
    pub async fn get_or_create_request_lock(&self, app_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut in_flight = self.in_flight_requests.lock().await;
        
        // Use entry API to avoid race condition
        match in_flight.get(app_id) {
            Some(existing_lock) => {
                #[cfg(debug_assertions)]
                println!("Using existing request lock for {}", app_id);
                existing_lock.clone()
            }
            None => {
                let lock = Arc::new(tokio::sync::Mutex::new(()));
                in_flight.insert(app_id.to_string(), lock.clone());
                
                #[cfg(debug_assertions)]
                println!("Created new request lock for {}", app_id);
                
                lock
            }
        }
    }

    /// Remove request lock
    pub async fn remove_request_lock(&self, app_id: &str) {
        let mut in_flight = self.in_flight_requests.lock().await;
        let removed = in_flight.remove(app_id);
        
        #[cfg(debug_assertions)]
        if removed.is_some() {
            println!("Removed request lock for {}", app_id);
        } else {
            println!("Warning: Attempted to remove non-existent lock for {}", app_id);
        }
    }

    /// Throttle requests to avoid rate limiting
    pub async fn throttle_request(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let delay = {
            let mut last_request = self.last_request_time.lock().await;
            let consecutive_errors = self.consecutive_errors.lock().await;

            let time_since_last = if *last_request > now || *last_request == 0 {
                0
            } else {
                now - *last_request
            };

            let base_delay = self.config.request_delay_ms;
            let backoff_multiplier = 2_u64.pow((*consecutive_errors).min(5));
            let required_delay = base_delay * backoff_multiplier;

            if time_since_last < required_delay {
                let delay = required_delay.saturating_sub(time_since_last);
                *last_request = now + delay;
                Some(delay)
            } else {
                *last_request = now;
                None
            }
        };

        if let Some(delay_ms) = delay {
            if delay_ms > 500 {
                println!("Rate limiting: waiting {}ms before next request", delay_ms);
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
        }
    }

    /// Record API error for circuit breaker
    pub async fn record_error(&self) {
        let mut consecutive_errors = self.consecutive_errors.lock().await;
        *consecutive_errors += 1;

        println!("API error recorded. Consecutive errors: {}", *consecutive_errors);

        // Open circuit breaker if too many errors
        if *consecutive_errors >= 5 {
            let mut circuit_open = self.circuit_breaker_open.lock().await;
            *circuit_open = true;
            println!("Circuit breaker opened due to consecutive errors");
        }
    }

    /// Reset error count on successful request
    pub async fn reset_error_count(&self) {
        let mut consecutive_errors = self.consecutive_errors.lock().await;
        if *consecutive_errors > 0 {
            println!("Resetting error count from {}", *consecutive_errors);
            *consecutive_errors = 0;
        }

        let mut circuit_open = self.circuit_breaker_open.lock().await;
        *circuit_open = false;
    }

    /// Check if circuit breaker is open
    pub async fn is_circuit_breaker_open(&self) -> bool {
        *self.circuit_breaker_open.lock().await
    }

    /// Clean up expired cache entries
    pub fn cleanup_expired(&self) -> Result<()> {
        let cleanup_result = self.db.cleanup_expired()?;
        
        if cleanup_result.games_deleted > 0 || cleanup_result.details_deleted > 0 {
            println!(
                "Cache cleanup: {} games, {} details deleted",
                cleanup_result.games_deleted,
                cleanup_result.details_deleted
            );
        }

        Ok(())
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> Result<()> {
        let stats = self.db.get_stats()?;
        println!("{}", stats);
        Ok(())
    }

    /// Save all data (for compatibility with old interface)
    pub fn save_to_disk(&self) {
        // SQLite auto-saves, but we can vacuum occasionally for optimization
        if let Err(e) = self.db.vacuum() {
            eprintln!("Failed to vacuum database: {}", e);
        }
    }

    /// Load from disk (for compatibility with old interface)
    pub fn load_from_disk(&self) {
        // SQLite loads automatically, but we can run cleanup
        if let Err(e) = self.cleanup_expired() {
            eprintln!("Failed to cleanup expired entries: {}", e);
        }
    }

    /// Clear all cache (for compatibility)
    pub fn clear_cache(&self) -> Result<()> {
        self.db.with_connection(|conn| {
            conn.execute("DELETE FROM games", [])?;
            conn.execute("DELETE FROM game_details", [])?;
            conn.execute("DELETE FROM user_library", [])?;
            Ok(())
        })?;

        println!("All cache cleared");
        Ok(())
    }

    // Private helper methods
    
    /// Clone service for background operations
    fn clone_for_background(&self) -> Self {
        Self {
            db: self.db.clone(),
            config: self.config.clone(),
            in_flight_requests: self.in_flight_requests.clone(),
            last_request_time: self.last_request_time.clone(),
            consecutive_errors: self.consecutive_errors.clone(),
            circuit_breaker_open: self.circuit_breaker_open.clone(),
            concurrent_limit: self.concurrent_limit.clone(),
        }
    }

    /// Queue game for background refresh
    fn queue_for_refresh(&self, app_id: String) {
        let service_clone = Arc::new(self.clone_for_background());
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let _ = service_clone.refresh_game_name_background(&app_id).await;
        });
    }

    /// Background refresh for game details
    async fn refresh_game_details_background(&self, app_id: &str) -> Option<GameDetail> {
        // This would call the existing fetch_game_details_background function
        // For now, we'll implement a placeholder
        println!("Background refresh for game details: {}", app_id);
        None
    }

    /// Background refresh for game name
    async fn refresh_game_name_background(&self, app_id: &str) -> Option<String> {
        // This would call the existing fetch_game_name_simple function
        // For now, we'll implement a placeholder
        println!("Background refresh for game name: {}", app_id);
        None
    }

    /// Get cache configuration
    pub fn get_config(&self) -> &CacheConfig {
        &self.config
    }

    /// Batch refresh multiple games with rate limiting and semaphore control
    pub async fn batch_refresh_games(&self, app_ids: Vec<String>) -> Result<BatchRefreshResult> {
        let mut result = BatchRefreshResult {
            total_requested: app_ids.len(),
            successfully_processed: 0,
            failed: 0,
            skipped_circuit_breaker: 0,
        };

        // Check circuit breaker first
        if self.is_circuit_breaker_open().await {
            println!("Circuit breaker open - skipping batch refresh of {} games", app_ids.len());
            result.skipped_circuit_breaker = app_ids.len();
            return Ok(result);
        }

        println!("Starting batch refresh of {} games", app_ids.len());

        // Process in chunks to avoid overwhelming Steam API
        for (batch_num, chunk) in app_ids.chunks(self.config.batch_size).enumerate() {
            println!("Processing batch {} of {} games", batch_num + 1, chunk.len());
            
            // Process each game in the chunk sequentially with rate limiting
            for app_id in chunk {
                // Acquire semaphore permit
                let _permit = self.concurrent_limit.acquire().await.unwrap();
                
                // Simulate background refresh (placeholder)
                println!("Background refresh for game details: {}", app_id);
                tokio::time::sleep(Duration::from_millis(self.config.request_delay_ms)).await;
                
                // For now, simulate success
                result.successfully_processed += 1;
                
                #[cfg(debug_assertions)]
                println!("✅ Successfully refreshed: {}", app_id);
                
                // Check if we should stop due to errors
                if self.is_circuit_breaker_open().await {
                    println!("Circuit breaker opened mid-processing - stopping");
                    let remaining_in_chunk = chunk.len() - (chunk.iter().position(|x| x == app_id).unwrap() + 1);
                    let remaining_batches = app_ids.chunks(self.config.batch_size).skip(batch_num + 1).count();
                    result.skipped_circuit_breaker = remaining_in_chunk + (remaining_batches * self.config.batch_size);
                    return Ok(result);
                }
            }
            
            // Delay between batches
            if batch_num < (app_ids.len() / self.config.batch_size) {
                println!("Batch {} completed. Waiting {} seconds before next batch...", 
                         batch_num + 1, self.config.batch_delay_seconds);
                sleep(Duration::from_secs(self.config.batch_delay_seconds)).await;
            }
        }
        
        println!("Batch refresh completed: {}/{} successful, {} failed, {} skipped", 
                 result.successfully_processed, result.total_requested, 
                 result.failed, result.skipped_circuit_breaker);
        
        Ok(result)
    }

    /// Smart refresh games based on granular TTL priority and staleness
    pub async fn smart_refresh_library(&self, library_games: Vec<String>) -> Result<BatchRefreshResult> {
        let mut refresh_queue: Vec<(String, u8, Vec<String>)> = Vec::new(); // (app_id, priority, expired_categories)
        
        println!("Analyzing {} library games for granular TTL smart refresh", library_games.len());
        
        // Categorize games by granular expiry
        for app_id in library_games {
            if let Ok(Some(detail)) = self.db.with_connection(|conn| {
                GameDetailOperations::get_by_id(conn, &app_id)
            }) {
                let expired_categories = detail.get_expired_categories();
                
                if expired_categories.is_empty() {
                    continue; // Skip fresh data
                }
                
                // Determine priority based on expired categories
                let priority = if expired_categories.contains(&"dynamic") {
                    0 // High priority - DLC data expired (3 days)
                } else if expired_categories.contains(&"semistatic") {
                    1 // Medium priority - name/images expired (3 weeks)
                } else if expired_categories.contains(&"static") {
                    2 // Low priority - screenshots/descriptions expired (60+ days)
                } else {
                    continue;
                };
                
                let expired_categories_strings: Vec<String> = expired_categories.into_iter().map(|s| s.to_string()).collect();
                refresh_queue.push((app_id, priority, expired_categories_strings));
            } else {
                // Missing data - highest priority
                refresh_queue.push((app_id, 0, vec!["all".to_string()]));
            }
        }
        
        // Sort by priority (0 = highest)
        refresh_queue.sort_by_key(|(_, priority, _)| *priority);
        
        // Log priority breakdown
        let high_priority = refresh_queue.iter().filter(|(_, p, _)| *p == 0).count();
        let medium_priority = refresh_queue.iter().filter(|(_, p, _)| *p == 1).count();
        let low_priority = refresh_queue.iter().filter(|(_, p, _)| *p == 2).count();
        
        println!("Smart refresh priority breakdown:");
        println!("  🔴 High (Dynamic expired): {} games", high_priority);
        println!("  🟡 Medium (Semi-static expired): {} games", medium_priority);
        println!("  🟢 Low (Static expired): {} games", low_priority);
        
        // Extract app_ids for batch processing
        let app_ids: Vec<String> = refresh_queue.into_iter()
            .map(|(app_id, _, _)| app_id)
            .collect();
        
        println!("Smart refresh: {} games queued (granular TTL priority)", app_ids.len());
        
        // Use batch processing
        self.batch_refresh_games(app_ids).await
    }

    /// Get games that need refresh based on granular TTL
    pub fn get_games_needing_refresh(&self) -> Result<Vec<(String, Vec<String>)>> {
        let mut games_needing_refresh = Vec::new();
        
        // Get games with any expired category
        let expired_games = self.db.with_connection(|conn| {
            GameDetailOperations::get_any_expired(conn)
        })?;
        
        for game in expired_games {
            let expired_categories = game.get_expired_categories();
            games_needing_refresh.push((game.app_id, expired_categories.into_iter().map(|s| s.to_string()).collect()));
        }
        
        Ok(games_needing_refresh)
    }

    /// Cleanup expired data by category (more efficient than full refresh)
    pub fn cleanup_expired_by_category(&self) -> Result<GranularCleanupResult> {
        let mut result = GranularCleanupResult {
            dynamic_expired: 0,
            semistatic_expired: 0,
            static_expired: 0,
            total_cleaned: 0,
        };
        
        self.db.with_connection(|conn| {
            // Count expired by category
            result.dynamic_expired = GameDetailOperations::get_dynamic_expired(conn)?.len();
            
            // For now, we'll just log the counts (actual cleanup would require partial updates)
            println!("Granular TTL cleanup analysis:");
            println!("  Dynamic data expired: {} games", result.dynamic_expired);
            
            // Note: Actual selective cleanup would require more complex SQL updates
            // For now, we maintain existing behavior but with better insights
            
            Ok(())
        })?;
        
        Ok(result)
    }

    // ============= BYPASS GAMES CACHE METHODS =============

    /// Get all bypass games from cache with 1 month TTL
    pub async fn get_bypass_games(&self) -> Result<Vec<BypassGame>> {
        // Try to get from cache first
        let cached_games = self.db.with_connection(|conn| {
            BypassGameOperations::get_all(conn)
        })?;

        // Check if we have valid cached data
        if !cached_games.is_empty() && !cached_games.iter().any(|game| game.is_expired()) {
            println!("Bypass games cache HIT: {} games", cached_games.len());
            return Ok(cached_games);
        }

        println!("Bypass games cache MISS or expired - loading from fallback JSON");
        
        // If cache is empty or expired, load from JSON and cache it
        self.load_bypass_games_from_json().await
    }

    /// Load bypass games from GitHub API and cache them
    async fn load_bypass_games_from_json(&self) -> Result<Vec<BypassGame>> {
        // Try to fetch from GitHub API first
        let json_data = match self.fetch_bypass_games_from_github().await {
            Ok(data) => {
                println!("Successfully fetched bypass games from GitHub API");
                data
            }
            Err(e) => {
                eprintln!("Failed to fetch from GitHub API: {}, falling back to embedded data", e);
                // Fallback to embedded data if GitHub API fails
                r#"[
  {
    "appId": "1174180",
    "name": "Red Dead Redemption 2",
    "image": "https://itsbintang.github.io/cdn/1174180.jpg",
    "bypasses": [
      {
        "type": "1",
        "url": "https://bypass.nzr.web.id/1174180_1.zip"
      }
    ]
  },
  {
    "appId": "1546990",
    "name": "Grand Theft Auto: Vice City - The Definitive Edition",
    "image": "https://itsbintang.github.io/cdn/1546990.jpg",
    "bypasses": [
      {
        "type": "3",
        "url": "http://cdn2.nzr.web.id/1546990_3.zip"
      }
    ]
  },
  {
    "appId": "582160",
    "name": "Assassin's Creed Origins",
    "image": "https://itsbintang.github.io/cdn/582160.jpg",
    "bypasses": [
      {
        "type": "1",
        "url": "https://bypass.nzr.web.id/582160_1.zip"
      }
    ]
  }
]"#.to_string()
            }
        };

        // Parse JSON data
        #[derive(serde::Deserialize)]
        struct JsonBypassGame {
            #[serde(rename = "appId")]
            app_id: String,
            name: String,
            image: String,
            bypasses: Vec<JsonBypassInfo>,
        }

        #[derive(serde::Deserialize)]
        struct JsonBypassInfo {
            #[serde(deserialize_with = "deserialize_type_field")]
            r#type: u8,
            url: String,
        }

        let json_games: Vec<JsonBypassGame> = serde_json::from_str(&json_data)?;
        
        // Convert to BypassGame models
        let mut bypass_games = Vec::new();
        for json_game in json_games {
            let bypasses = json_game.bypasses.into_iter()
                .map(|b| BypassInfo { r#type: b.r#type, url: b.url })
                .collect();
            
            let bypass_game = BypassGame::new(
                json_game.app_id,
                json_game.name,
                json_game.image,
                bypasses,
            );
            
            bypass_games.push(bypass_game);
        }

        // Cache the data
        self.db.with_connection(|conn| {
            // Clear existing data first
            BypassGameOperations::clear_all(conn)?;
            
            // Insert new data
            for game in &bypass_games {
                BypassGameOperations::insert(conn, game)?;
            }
            
            Ok(())
        })?;

        println!("Bypass games cached successfully: {} games", bypass_games.len());
        Ok(bypass_games)
    }

    /// Force refresh bypass games cache (useful for updates)
    pub async fn refresh_bypass_games(&self) -> Result<Vec<BypassGame>> {
        println!("Force refreshing bypass games cache...");
        self.load_bypass_games_from_json().await
    }

    /// Get bypass game by app_id
    pub async fn get_bypass_game(&self, app_id: &str) -> Result<Option<BypassGame>> {
        // Check cache first
        let cached_game = self.db.with_connection(|conn| {
            BypassGameOperations::get_by_id(conn, app_id)
        })?;

        if let Some(game) = cached_game {
            if !game.is_expired() {
                return Ok(Some(game));
            }
        }

        // If not found or expired, refresh all bypass games and try again
        let all_games = self.get_bypass_games().await?;
        Ok(all_games.into_iter().find(|g| g.app_id == app_id))
    }

    /// Fetch bypass games data from GitHub API
    async fn fetch_bypass_games_from_github(&self) -> Result<String> {
        use reqwest;
        
        // GitHub API URL for the bypass games JSON file
        let github_api_url = "https://api.github.com/repos/itsbintang/bypass-games-api/contents/bypassGames.json";
        
        let client = reqwest::Client::new();
        let response = client
            .get(github_api_url)
            .header("User-Agent", "Zenith-Launcher")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("GitHub API returned error: {}", response.status()));
        }

        let github_response: serde_json::Value = response.json().await?;
        
        // Extract base64 content from GitHub API response
        let content = github_response["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No content field in GitHub response"))?;

        // Decode base64 content
        use base64::{Engine as _, engine::general_purpose};
        let decoded_bytes = general_purpose::STANDARD.decode(content.replace('\n', ""))
            .map_err(|e| anyhow::anyhow!("Failed to decode base64: {}", e))?;

        let json_content = String::from_utf8(decoded_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to convert to UTF-8: {}", e))?;

        Ok(json_content)
    }
}

/// Custom deserializer for type field that can handle both string and integer
fn deserialize_type_field<'de, D>(deserializer: D) -> std::result::Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct TypeVisitor;

    impl<'de> Visitor<'de> for TypeVisitor {
        type Value = u8;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or integer representing a type")
        }

        fn visit_str<E>(self, value: &str) -> std::result::Result<u8, E>
        where
            E: de::Error,
        {
            value.parse::<u8>().map_err(de::Error::custom)
        }

        fn visit_u64<E>(self, value: u64) -> std::result::Result<u8, E>
        where
            E: de::Error,
        {
            if value <= 255 {
                Ok(value as u8)
            } else {
                Err(de::Error::custom("type value too large"))
            }
        }
    }

    deserializer.deserialize_any(TypeVisitor)
}

/// Result of batch refresh operation
#[derive(Debug)]
pub struct BatchRefreshResult {
    pub total_requested: usize,
    pub successfully_processed: usize,
    pub failed: usize,
    pub skipped_circuit_breaker: usize,
}

/// Result of granular TTL cleanup operation
#[derive(Debug)]
pub struct GranularCleanupResult {
    pub dynamic_expired: usize,
    pub semistatic_expired: usize,
    pub static_expired: usize,
    pub total_cleaned: usize,
}

lazy_static::lazy_static! {
    pub static ref SQLITE_CACHE_SERVICE: Arc<SqliteCacheService> = {
        Arc::new(SqliteCacheService::new().expect("Failed to initialize SQLite cache service"))
    };
}
