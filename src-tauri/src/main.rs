// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod models;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use futures::stream::{self, StreamExt};
use regex::Regex;
use std::process::Command;
use tauri::{command, Emitter};
use tauri_plugin_updater::UpdaterExt;
use tempfile::TempDir;
use tokio::time::sleep;
use walkdir::WalkDir;
use zip::ZipArchive;

#[cfg(target_os = "windows")]
use winreg::{enums::*, RegKey};

fn sanitize_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_alphanumeric() || ch == ' ' || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

#[derive(Debug, Serialize, Deserialize)]
struct DownloadResult {
    success: bool,
    message: String,
    file_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct BypassProgress {
    step: String,
    progress: f64,
    app_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BypassInfo {
    url: String,
    size: u64,
    available: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct BypassStatus {
    available: bool,
    installing: bool,
    installed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct BypassResult {
    success: bool,
    message: String,
    should_launch: bool,
    game_executable_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry<T> {
    data: T,
    timestamp: u64,
    expires_at: u64,
}

impl<T> CacheEntry<T> {
    fn new(data: T, ttl_seconds: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            data,
            timestamp: now,
            expires_at: now + ttl_seconds,
        }
    }

    fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now > self.expires_at
    }
}

struct GameCache {
    game_details: Arc<Mutex<HashMap<String, CacheEntry<GameDetail>>>>,
    game_names: Arc<Mutex<HashMap<String, CacheEntry<String>>>>,
    in_flight_requests: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    last_request_time: Arc<Mutex<u64>>,
    request_delay_ms: u64,
    cache_dir: PathBuf,
    consecutive_errors: Arc<Mutex<u32>>,
    last_error_time: Arc<Mutex<u64>>,
    request_queue: Arc<Mutex<Vec<String>>>,
    is_processing_queue: Arc<Mutex<bool>>,
    circuit_breaker_open: Arc<Mutex<bool>>,
    circuit_breaker_failures: Arc<Mutex<u32>>,
}

impl GameCache {
    fn new() -> Self {
        // Create cache directory in app data folder
        let cache_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zenith-launcher")
            .join("cache");

        // Ensure cache directory exists
        let _ = fs::create_dir_all(&cache_dir);

        let mut instance = Self {
            game_details: Arc::new(Mutex::new(HashMap::new())),
            game_names: Arc::new(Mutex::new(HashMap::new())),
            in_flight_requests: Arc::new(Mutex::new(HashMap::new())),
            last_request_time: Arc::new(Mutex::new(0)),
            request_delay_ms: 200, // Increased to 200ms between requests for safety
            cache_dir: cache_dir.clone(),
            consecutive_errors: Arc::new(Mutex::new(0)),
            last_error_time: Arc::new(Mutex::new(0)),
            request_queue: Arc::new(Mutex::new(Vec::new())),
            is_processing_queue: Arc::new(Mutex::new(false)),
            circuit_breaker_open: Arc::new(Mutex::new(false)),
            circuit_breaker_failures: Arc::new(Mutex::new(0)),
        };

        // Load cache from disk
        instance.load_from_disk();

        instance
    }

    async fn get_game_details(&self, app_id: &str) -> Option<GameDetail> {
        let cache = self.game_details.lock().unwrap();
        if let Some(entry) = cache.get(app_id) {
            if !entry.is_expired() {
                return Some(entry.data.clone());
            } else {
                // Return stale data while revalidating in background
                let stale_data = entry.data.clone();
                drop(cache);

                // Queue for background refresh (game details)
                let app_id_clone = app_id.to_string();
                tokio::spawn(async move {
                    // Refresh in background
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    let _ = fetch_game_details_background(&app_id_clone).await;
                });

                return Some(stale_data);
            }
        }
        None
    }

    fn set_game_details(&self, app_id: String, details: GameDetail) {
        let mut cache = self.game_details.lock().unwrap();
        cache.insert(app_id.clone(), CacheEntry::new(details, 86400)); // 1 day TTL
        drop(cache); // Release lock before saving
        self.save_to_disk();

        // Only log in debug mode
        #[cfg(debug_assertions)]
        println!("Cached game details for {}", app_id);
    }

    async fn get_game_name(&self, app_id: &str) -> Option<String> {
        let cache = self.game_names.lock().unwrap();
        if let Some(entry) = cache.get(app_id) {
            if !entry.is_expired() {
                return Some(entry.data.clone());
            } else {
                // Return stale data while revalidating in background
                let stale_data = entry.data.clone();
                drop(cache);

                // Queue for background refresh
                self.queue_for_refresh(app_id.to_string());

                // Return stale data immediately (stale-while-revalidate)
                return Some(stale_data);
            }
        }
        None
    }

    fn set_game_name(&self, app_id: String, name: String) {
        let mut cache = self.game_names.lock().unwrap();
        cache.insert(app_id.clone(), CacheEntry::new(name.clone(), 604800)); // 7 days TTL
        drop(cache); // Release lock before saving
        self.save_to_disk();

        // Only log in debug mode
        #[cfg(debug_assertions)]
        println!("Cached game name for {}: {}", app_id, name);
    }

    async fn get_or_create_request_lock(&self, app_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut in_flight = self.in_flight_requests.lock().unwrap();
        if let Some(lock) = in_flight.get(app_id) {
            lock.clone()
        } else {
            let lock = Arc::new(tokio::sync::Mutex::new(()));
            in_flight.insert(app_id.to_string(), lock.clone());
            lock
        }
    }

    fn remove_request_lock(&self, app_id: &str) {
        let mut in_flight = self.in_flight_requests.lock().unwrap();
        in_flight.remove(app_id);
    }

    async fn throttle_request(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Calculate delay with exponential backoff
        let delay = {
            let mut last_request = self.last_request_time.lock().unwrap();
            let consecutive_errors = self.consecutive_errors.lock().unwrap();

            // Handle case where last_request might be 0 or greater than now
            let time_since_last = if *last_request > now || *last_request == 0 {
                // First request or clock issue - no delay needed
                0
            } else {
                now - *last_request
            };

            // Base delay with exponential backoff based on consecutive errors
            let base_delay = self.request_delay_ms;
            let backoff_multiplier = 2_u64.pow((*consecutive_errors).min(5)); // Cap at 2^5 = 32x
            let required_delay = base_delay * backoff_multiplier;

            if time_since_last < required_delay {
                let delay = required_delay.saturating_sub(time_since_last);
                // Update the last request time now to prevent race conditions
                *last_request = now + delay;
                Some(delay)
            } else {
                *last_request = now;
                None
            }
        }; // Lock is dropped here

        if let Some(delay_ms) = delay {
            // Only log significant delays
            if delay_ms > 500 {
                println!("Rate limiting: waiting {}ms before next request", delay_ms);
            }
            sleep(Duration::from_millis(delay_ms)).await;
        }
    }

    fn record_error(&self) {
        let mut consecutive_errors = self.consecutive_errors.lock().unwrap();
        *consecutive_errors += 1;

        let mut last_error_time = self.last_error_time.lock().unwrap();
        *last_error_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Increment circuit breaker failures
        let mut cb_failures = self.circuit_breaker_failures.lock().unwrap();
        *cb_failures += 1;
        drop(cb_failures);

        println!(
            "API error recorded. Consecutive errors: {}",
            *consecutive_errors
        );

        // Check if we should open circuit breaker
        self.check_circuit_breaker();
    }

    fn reset_error_count(&self) {
        let mut consecutive_errors = self.consecutive_errors.lock().unwrap();
        if *consecutive_errors > 0 {
            println!("Resetting error count from {}", *consecutive_errors);
            *consecutive_errors = 0;
        }
    }

    fn cleanup_expired(&self) {
        // Clean up expired entries
        let mut details_cache = self.game_details.lock().unwrap();
        let details_before = details_cache.len();
        details_cache.retain(|_, entry| !entry.is_expired());
        let details_after = details_cache.len();

        let mut names_cache = self.game_names.lock().unwrap();
        let names_before = names_cache.len();
        names_cache.retain(|_, entry| !entry.is_expired());
        let names_after = names_cache.len();

        // Only log if something was actually cleaned
        if details_before != details_after || names_before != names_after {
            println!(
                "Cache cleanup: Details {}->{}, Names {}->{}",
                details_before, details_after, names_before, names_after
            );
        }
    }

    fn cache_stats(&self) {
        let details_cache = self.game_details.lock().unwrap();
        let names_cache = self.game_names.lock().unwrap();
        let in_flight = self.in_flight_requests.lock().unwrap();

        println!(
            "Cache stats: {} game details, {} game names, {} in-flight requests",
            details_cache.len(),
            names_cache.len(),
            in_flight.len()
        );
    }

    fn save_to_disk(&self) {
        // Save game names cache
        let names_cache = self.game_names.lock().unwrap();
        if !names_cache.is_empty() {
            let names_path = self.cache_dir.join("game_names.json");
            if let Ok(json) = serde_json::to_string(&*names_cache) {
                let _ = fs::write(names_path, json);
            }
        }
        drop(names_cache);

        // Save game details cache
        let details_cache = self.game_details.lock().unwrap();
        if !details_cache.is_empty() {
            let details_path = self.cache_dir.join("game_details.json");
            if let Ok(json) = serde_json::to_string(&*details_cache) {
                let _ = fs::write(details_path, json);
            }
        }
    }

    fn load_from_disk(&mut self) {
        // Load game names cache
        let names_path = self.cache_dir.join("game_names.json");
        if names_path.exists() {
            if let Ok(content) = fs::read_to_string(&names_path) {
                if let Ok(cached_names) =
                    serde_json::from_str::<HashMap<String, CacheEntry<String>>>(&content)
                {
                    let mut names_cache = self.game_names.lock().unwrap();
                    // Only load non-expired entries
                    for (key, entry) in cached_names {
                        if !entry.is_expired() {
                            names_cache.insert(key, entry);
                        }
                    }
                    println!("Loaded {} game names from cache", names_cache.len());
                }
            }
        }

        // Load game details cache
        let details_path = self.cache_dir.join("game_details.json");
        if details_path.exists() {
            if let Ok(content) = fs::read_to_string(&details_path) {
                if let Ok(cached_details) =
                    serde_json::from_str::<HashMap<String, CacheEntry<GameDetail>>>(&content)
                {
                    let mut details_cache = self.game_details.lock().unwrap();
                    // Only load non-expired entries
                    for (key, entry) in cached_details {
                        if !entry.is_expired() {
                            details_cache.insert(key, entry);
                        }
                    }
                    println!("Loaded {} game details from cache", details_cache.len());
                }
            }
        }
    }

    fn clear_all(&self) {
        // Clear in-memory cache
        {
            let mut details_cache = self.game_details.lock().unwrap();
            details_cache.clear();
        }
        {
            let mut names_cache = self.game_names.lock().unwrap();
            names_cache.clear();
        }

        // Clear disk cache
        let _ = fs::remove_file(self.cache_dir.join("game_names.json"));
        let _ = fs::remove_file(self.cache_dir.join("game_details.json"));

        println!("Cache cleared completely");
    }

    fn queue_for_refresh(&self, app_id: String) {
        let mut queue = self.request_queue.lock().unwrap();
        if !queue.contains(&app_id) {
            queue.push(app_id);
        }
    }

    async fn process_queue_batch(&self) {
        // Check if already processing
        {
            let mut is_processing = self.is_processing_queue.lock().unwrap();
            if *is_processing {
                return;
            }
            *is_processing = true;
        } // Drop lock before async operations

        // Process queue in batches
        loop {
            // Check circuit breaker
            let is_circuit_open = { *self.circuit_breaker_open.lock().unwrap() };

            if is_circuit_open {
                println!("Circuit breaker is open, pausing queue processing");
                sleep(Duration::from_secs(30)).await; // Wait 30s before retry
                self.try_close_circuit_breaker();
                continue;
            }

            // Get next batch (max 5 items)
            let batch: Vec<String> = {
                let mut queue = self.request_queue.lock().unwrap();
                let batch_size = queue.len().min(5);
                if batch_size == 0 {
                    break; // Queue is empty
                }
                queue.drain(0..batch_size).collect()
            };

            // Process batch with rate limiting
            for app_id in batch {
                self.throttle_request().await;

                // Fetch in background without blocking
                let app_id_clone = app_id.clone();
                tokio::spawn(async move {
                    fetch_game_name_simple(&app_id_clone).await;
                });

                // Extra delay between batch items
                sleep(Duration::from_millis(100)).await;
            }

            // Pause between batches
            sleep(Duration::from_secs(2)).await;
        }

        // Mark as not processing
        {
            *self.is_processing_queue.lock().unwrap() = false;
        }
    }

    fn open_circuit_breaker(&self) {
        let mut is_open = self.circuit_breaker_open.lock().unwrap();
        if !*is_open {
            *is_open = true;
            println!("⚠️ Circuit breaker opened - too many API failures");
        }
    }

    fn try_close_circuit_breaker(&self) {
        let mut failures = self.circuit_breaker_failures.lock().unwrap();
        if *failures > 0 {
            *failures = (*failures).saturating_sub(1);
            if *failures == 0 {
                *self.circuit_breaker_open.lock().unwrap() = false;
                println!("✅ Circuit breaker closed - resuming normal operation");
            }
        }
    }

    fn check_circuit_breaker(&self) {
        let failures = *self.circuit_breaker_failures.lock().unwrap();
        if failures >= 5 {
            self.open_circuit_breaker();
        }
    }
}

lazy_static::lazy_static! {
    static ref GAME_CACHE: GameCache = GameCache::new();
    static ref HTTP_CLIENT: reqwest::Client = {
        reqwest::Client::builder()
            .user_agent("zenith-launcher/1.0")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client")
    };
    static ref DOWNLOAD_CLIENT: reqwest::Client = {
        reqwest::Client::builder()
            .user_agent("zenith-launcher/1.0")
            .timeout(Duration::from_secs(600)) // 10 minutes for large downloads
            .connect_timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create download HTTP client")
    };
}

#[derive(Debug, Serialize, Deserialize)]
struct GameInfo {
    app_id: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SearchResultItem {
    app_id: String,
    name: String,
    header_image: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GameDetail {
    app_id: String,
    name: String,
    header_image: String,
    banner_image: String,
    detailed_description: String,
    release_date: String,
    publisher: String,
    trailer: Option<String>,
    screenshots: Vec<String>,
    sysreq_min: Vec<(String, String)>,
    sysreq_rec: Vec<(String, String)>,
    pc_requirements: Option<PcRequirements>,
    dlc: Vec<String>,           // List of DLC AppIDs
    drm_notice: Option<String>, // DRM information from Steam API
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PcRequirements {
    minimum: Option<String>,
    recommended: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct LibraryGame {
    app_id: String,
    name: String,
    header_image: String,
}

#[derive(Debug, Deserialize)]
struct SearchAppsRawItem {
    appid: Option<u32>,
    name: Option<String>,
}

// Repository types we support
#[derive(Debug, Clone)]
enum RepoType {
    Branch,
    Decrypted,
    DirectZip, // For direct ZIP file downloads like Furcate.eu
    DirectUrl, // For direct URL downloads with specific patterns
}

#[command]
async fn download_game(
    app_id: String,
    game_name: String,
    save_zip: Option<bool>,
    save_dir: Option<String>,
) -> Result<DownloadResult, String> {
    println!(
        "Starting seamless installation for AppID: {} ({})",
        app_id, game_name
    );
    let should_save_zip = save_zip.unwrap_or(false);
    let save_directory: Option<PathBuf> = save_dir.clone().and_then(|p| {
        let pb = PathBuf::from(p);
        if pb.exists() {
            Some(pb)
        } else {
            None
        }
    });

    // Setup repositories to try (in priority order)
    let mut repos = Vec::new();
    // Prioritas pertama
    repos.push(("https://furcate.eu/FILES/".to_string(), RepoType::DirectZip));
    repos.push(("Fairyvmos/bruh-hub".to_string(), RepoType::Branch));
    repos.push(("SteamAutoCracks/ManifestHub".to_string(), RepoType::Branch));
    repos.push(("itsBintang/ManifestHub".to_string(), RepoType::Branch));
    repos.push((
        "https://raw.githubusercontent.com/sushi-dev55/sushitools-games-repo/refs/heads/main/"
            .to_string(),
        RepoType::DirectZip,
    ));
    repos.push((
        "http://masss.pythonanywhere.com/storage?auth=IEOIJE54esfsipoE56GE4&appid=".to_string(),
        RepoType::DirectUrl,
    ));
    repos.push((
        "https://mellyiscoolaf.pythonanywhere.com/".to_string(),
        RepoType::DirectUrl,
    ));

    // Use global HTTP client

    // Try downloading from repositories
    for (repo_name, repo_type) in &repos {
        println!("Trying repository: {}", repo_name);

        match repo_type {
            RepoType::DirectZip => {
                let download_url = format!("{}{}.zip", repo_name, app_id);
                println!("Downloading from: {}", download_url);

                match HTTP_CLIENT
                    .get(&download_url)
                    .timeout(std::time::Duration::from_secs(60))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status().is_success() {
                            let bytes = response.bytes().await.map_err(|e| e.to_string())?;
                            if should_save_zip {
                                if let Some(dir) = save_directory.clone() {
                                    let filename =
                                        format!("{}-{}.zip", app_id, sanitize_filename(&game_name));
                                    let path = dir.join(filename);
                                    if let Err(e) = fs::write(&path, &bytes) {
                                        println!("Failed to save ZIP to {:?}: {}", path, e);
                                    } else {
                                        println!("Saved ZIP to {:?}", path);
                                    }
                                }
                            }

                            // Process ZIP in memory and install to Steam
                            match process_and_install_to_steam(&bytes, &app_id, &game_name).await {
                                Ok(install_info) => {
                                    return Ok(DownloadResult {
                                        success: true,
                                        message: format!(
                                            "Successfully installed {} to Steam! {}",
                                            game_name, install_info
                                        ),
                                        file_path: None, // No local file saved
                                    });
                                }
                                Err(e) => {
                                    println!("Failed to install to Steam: {}", e);
                                    continue; // Try next repository
                                }
                            }
                        } else {
                            println!(
                                "Failed to download from {}: HTTP {}",
                                repo_name,
                                response.status()
                            );
                        }
                    }
                    Err(e) => {
                        println!("Error downloading from {}: {}", repo_name, e);
                    }
                }
            }
            RepoType::Branch => {
                let api_url = format!(
                    "https://api.github.com/repos/{}/zipball/{}",
                    repo_name, app_id
                );
                println!("Downloading from: {}", api_url);

                match HTTP_CLIENT
                    .get(&api_url)
                    .timeout(std::time::Duration::from_secs(60))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status().is_success() {
                            let bytes = response.bytes().await.map_err(|e| e.to_string())?;
                            if should_save_zip {
                                if let Some(dir) = save_directory.clone() {
                                    let filename =
                                        format!("{}-{}.zip", app_id, sanitize_filename(&game_name));
                                    let path = dir.join(filename);
                                    if let Err(e) = fs::write(&path, &bytes) {
                                        println!("Failed to save ZIP to {:?}: {}", path, e);
                                    } else {
                                        println!("Saved ZIP to {:?}", path);
                                    }
                                }
                            }

                            // Process ZIP in memory and install to Steam
                            match process_and_install_to_steam(&bytes, &app_id, &game_name).await {
                                Ok(install_info) => {
                                    return Ok(DownloadResult {
                                        success: true,
                                        message: format!(
                                            "Successfully installed {} to Steam! {}",
                                            game_name, install_info
                                        ),
                                        file_path: None, // No local file saved
                                    });
                                }
                                Err(e) => {
                                    println!("Failed to install to Steam: {}", e);
                                    continue; // Try next repository
                                }
                            }
                        } else {
                            println!(
                                "Failed to download from {}: HTTP {}",
                                repo_name,
                                response.status()
                            );
                        }
                    }
                    Err(e) => {
                        println!("Error downloading from {}: {}", repo_name, e);
                    }
                }
            }
            RepoType::DirectUrl => {
                let download_url = format!("{}{}", repo_name, app_id);
                println!("Downloading from: {}", download_url);

                match HTTP_CLIENT
                    .get(&download_url)
                    .timeout(std::time::Duration::from_secs(60))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status().is_success() {
                            let bytes = response.bytes().await.map_err(|e| e.to_string())?;
                            if should_save_zip {
                                if let Some(dir) = save_directory.clone() {
                                    let filename =
                                        format!("{}-{}.zip", app_id, sanitize_filename(&game_name));
                                    let path = dir.join(filename);
                                    if let Err(e) = fs::write(&path, &bytes) {
                                        println!("Failed to save ZIP to {:?}: {}", path, e);
                                    } else {
                                        println!("Saved ZIP to {:?}", path);
                                    }
                                }
                            }

                            // Process ZIP in memory and install to Steam
                            match process_and_install_to_steam(&bytes, &app_id, &game_name).await {
                                Ok(install_info) => {
                                    return Ok(DownloadResult {
                                        success: true,
                                        message: format!(
                                            "Successfully installed {} to Steam! {}",
                                            game_name, install_info
                                        ),
                                        file_path: None, // No local file saved
                                    });
                                }
                                Err(e) => {
                                    println!("Failed to install to Steam: {}", e);
                                    continue; // Try next repository
                                }
                            }
                        } else {
                            println!(
                                "Failed to download from {}: HTTP {}",
                                repo_name,
                                response.status()
                            );
                        }
                    }
                    Err(e) => {
                        println!("Error downloading from {}: {}", repo_name, e);
                    }
                }
            }
            _ => {
                println!("Repo type {:?} not implemented yet", repo_type);
            }
        }
    }

    // If we get here, all repositories failed
    Ok(DownloadResult {
        success: false,
        message: format!(
            "No data found for {} (AppID: {}) in any repository",
            game_name, app_id
        ),
        file_path: None,
    })
}

#[command]
async fn search_games(query: String) -> Result<Vec<SearchResultItem>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(vec![]);
    }

    println!("Searching for: '{}'", query);

    // Split query into search terms (support comma-separated)
    let search_terms: Vec<String> = query
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let mut all_results = Vec::new();

    // Process each search term
    for term in search_terms {
        // Check if it's a Steam URL and extract AppID
        if let Some(app_id) = extract_appid_from_url(&term) {
            println!("Detected Steam URL, extracted AppID: {}", app_id);
            let name = fetch_game_name_simple(&app_id)
                .await
                .unwrap_or_else(|| format!("Unknown Game ({})", app_id));
            let header = header_image_for(&app_id);
            all_results.push(SearchResultItem {
                app_id: app_id.clone(),
                name,
                header_image: header,
            });
        }
        // If numeric: treat as AppID direct
        else if term.chars().all(|c| c.is_ascii_digit()) {
            println!("Searching by AppID: {}", term);
            let name = fetch_game_name_simple(&term)
                .await
                .unwrap_or_else(|| format!("Unknown Game ({})", term));
            let header = header_image_for(&term);
            all_results.push(SearchResultItem {
                app_id: term.clone(),
                name,
                header_image: header,
            });
        } else {
            // Name search via Steam Store API (more reliable than community search)
            println!("Searching by name: {}", term);

            // Try multiple search approaches
            let search_results = search_steam_store(&term).await;
            all_results.extend(search_results);
        }
    }

    // Remove duplicates based on app_id
    let mut seen = std::collections::HashSet::new();
    let unique_results: Vec<SearchResultItem> = all_results
        .into_iter()
        .filter(|item| seen.insert(item.app_id.clone()))
        .take(50) // Limit total results
        .collect();

    println!("Found {} unique results", unique_results.len());
    Ok(unique_results)
}

// Helper function to search Steam store
async fn search_steam_store(query: &str) -> Vec<SearchResultItem> {
    let mut results = Vec::new();

    // Method 1: Try Steam Store search API
    let encoded_query = query.replace(' ', "+");
    let store_search_url = format!(
        "https://store.steampowered.com/api/storesearch/?term={}&l=english&cc=US",
        encoded_query
    );

    println!("Trying Steam Store API: {}", store_search_url);

    if let Ok(resp) = HTTP_CLIENT.get(&store_search_url).send().await {
        if resp.status().is_success() {
            if let Ok(json_value) = resp.json::<serde_json::Value>().await {
                if let Some(items) = json_value.get("items").and_then(|v| v.as_array()) {
                    for item in items.iter().take(20) {
                        if let (Some(id), Some(name)) = (
                            item.get("id").and_then(|v| v.as_u64()),
                            item.get("name").and_then(|v| v.as_str()),
                        ) {
                            let app_id = id.to_string();

                            // Filter out non-games unless explicitly searched for
                            let name_lower = name.to_lowercase();
                            let query_lower = query.to_lowercase();

                            let is_non_game = [
                                "dlc",
                                "soundtrack",
                                "demo",
                                "pack",
                                "sdk",
                                "artbook",
                                "trailer",
                                "movie",
                                "beta",
                                "ost",
                                "wallpaper",
                                "season pass",
                                "bonus content",
                                "pre-purchase",
                                "pre-order",
                                "expansion",
                            ]
                            .iter()
                            .any(|&keyword| name_lower.contains(keyword));

                            let searching_for_non_game = [
                                "dlc",
                                "soundtrack",
                                "demo",
                                "pack",
                                "artbook",
                                "trailer",
                                "movie",
                                "beta",
                                "pass",
                                "expansion",
                            ]
                            .iter()
                            .any(|&keyword| query_lower.contains(keyword));

                            if !is_non_game || searching_for_non_game {
                                results.push(SearchResultItem {
                                    app_id: app_id.clone(),
                                    name: name.to_string(),
                                    header_image: header_image_for(&app_id),
                                });
                            }
                        }
                    }
                    println!("Found {} results from Steam Store API", results.len());
                    return results;
                }
            }
        }
    }

    // Method 2: Fallback to community search if store search fails
    println!("Store search failed, trying community search");
    let encoded = query.replace(' ', "%20");
    let community_url = format!("https://steamcommunity.com/actions/SearchApps/{}", encoded);

    if let Ok(resp) = HTTP_CLIENT.get(&community_url).send().await {
        if resp.status().is_success() {
            if let Ok(raw) = resp.json::<Vec<SearchAppsRawItem>>().await {
                for item in raw.into_iter().take(15) {
                    if let (Some(id), Some(name)) = (item.appid, item.name) {
                        let app_id = id.to_string();

                        // Apply same filtering
                        let name_lower = name.to_lowercase();
                        let query_lower = query.to_lowercase();

                        let is_non_game = [
                            "dlc",
                            "soundtrack",
                            "demo",
                            "pack",
                            "sdk",
                            "artbook",
                            "trailer",
                            "movie",
                            "beta",
                            "ost",
                            "wallpaper",
                            "season pass",
                            "bonus content",
                            "pre-purchase",
                            "pre-order",
                        ]
                        .iter()
                        .any(|&keyword| name_lower.contains(keyword));

                        let searching_for_non_game = [
                            "dlc",
                            "soundtrack",
                            "demo",
                            "pack",
                            "artbook",
                            "trailer",
                            "movie",
                            "beta",
                            "pass",
                        ]
                        .iter()
                        .any(|&keyword| query_lower.contains(keyword));

                        if !is_non_game || searching_for_non_game {
                            results.push(SearchResultItem {
                                app_id: app_id.clone(),
                                name,
                                header_image: header_image_for(&app_id),
                            });
                        }
                    }
                }
                println!("Found {} results from community search", results.len());
            }
        }
    }

    results
}

#[derive(Debug, Serialize, Deserialize)]
struct InitProgress {
    step: String,
    progress: f32,
    completed: bool,
}

#[command]
async fn initialize_app() -> Result<Vec<InitProgress>, String> {
    let mut progress_steps = Vec::new();

    // Step 1: Check Steam installation
    progress_steps.push(InitProgress {
        step: "Checking Steam installation...".to_string(),
        progress: 20.0,
        completed: false,
    });

    match find_steam_config_path() {
        Ok(_) => {
            progress_steps.push(InitProgress {
                step: "Steam found successfully".to_string(),
                progress: 40.0,
                completed: true,
            });
        }
        Err(_) => {
            return Err("Steam installation not found. Please install Steam first.".to_string());
        }
    }

    // Step 2: Initialize cache system
    progress_steps.push(InitProgress {
        step: "Initializing cache system...".to_string(),
        progress: 60.0,
        completed: false,
    });

    // Cleanup any expired cache entries
    GAME_CACHE.cleanup_expired();
    GAME_CACHE.cache_stats();

    progress_steps.push(InitProgress {
        step: "Cache system ready".to_string(),
        progress: 80.0,
        completed: true,
    });

    // Step 3: Pre-load library with full game names (warm-up cache)
    progress_steps.push(InitProgress {
        step: "Loading game library...".to_string(),
        progress: 70.0,
        completed: false,
    });

    let steam_config_path = find_steam_config_path().map_err(|e| e.to_string())?;
    let stplugin_dir = steam_config_path.join("stplug-in");

    let game_count = if stplugin_dir.exists() {
        // Collect all numeric app IDs
        let mut app_ids = Vec::new();
        if let Ok(entries) = fs::read_dir(&stplugin_dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("lua") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if stem.chars().all(|c| c.is_ascii_digit()) {
                            app_ids.push(stem.to_string());
                        }
                    }
                }
            }
        }

        progress_steps.push(InitProgress {
            step: format!(
                "Pre-loading {} games (this ensures instant library access)...",
                app_ids.len()
            ),
            progress: 70.0,
            completed: false,
        });

        // Smart cache warming strategy
        if !app_ids.is_empty() {
            let mut uncached_ids = Vec::new();
            let mut cached_count = 0;

            for app_id in &app_ids {
                if GAME_CACHE.get_game_name(app_id).await.is_none() {
                    uncached_ids.push(app_id.clone());
                } else {
                    cached_count += 1;
                }
            }

            if cached_count > 0 {
                println!(
                    "Found {} cached games - library will load instantly!",
                    cached_count
                );
            }

            if !uncached_ids.is_empty() {
                // Strategy: Load more games during initialization when cache is invalid
                let priority_count = if uncached_ids.len() <= 8 {
                    uncached_ids.len() // Load all if small library
                } else if uncached_ids.len() <= 20 {
                    15 // Load 15 games for medium library
                } else {
                    20 // Load 20 games for large library
                };

                let (priority_ids, background_ids): (Vec<_>, Vec<_>) = uncached_ids
                    .into_iter()
                    .enumerate()
                    .partition(|(i, _)| *i < priority_count);

                let priority_ids: Vec<_> = priority_ids.into_iter().map(|(_, id)| id).collect();
                let background_ids: Vec<_> = background_ids.into_iter().map(|(_, id)| id).collect();

                println!(
                    "Cache warming: {} priority games (blocking), {} background games (queued)",
                    priority_ids.len(),
                    background_ids.len()
                );

                // Load priority games during initialization (blocking)
                if !priority_ids.is_empty() {
                    println!(
                        "🔄 Loading {} games during initialization (this may take a moment)...",
                        priority_ids.len()
                    );

                    let games: Vec<_> = stream::iter(priority_ids)
                        .map(|app_id: String| async move { fetch_game_name_simple(&app_id).await })
                        .buffer_unordered(4) // Increased concurrency for faster initialization
                        .collect()
                        .await;

                    let loaded_count = games.iter().filter(|g| g.is_some()).count();
                    println!(
                        "✅ Pre-loaded {} priority games during initialization",
                        loaded_count
                    );
                }

                // Queue remaining games for background processing
                if !background_ids.is_empty() {
                    for app_id in background_ids {
                        GAME_CACHE.queue_for_refresh(app_id);
                    }

                    // Start background processing (non-blocking)
                    tokio::spawn(async {
                        // Small delay to let app finish initializing
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        GAME_CACHE.process_queue_batch().await;
                    });

                    println!("🔄 Background cache warming started");
                }
            } else {
                println!(
                    "✨ All {} games already cached - instant loading!",
                    app_ids.len()
                );
            }
        }

        app_ids.len()
    } else {
        0
    };

    progress_steps.push(InitProgress {
        step: format!(
            "Ready! {} games pre-loaded for instant library access",
            game_count
        ),
        progress: 100.0,
        completed: true,
    });

    println!(
        "App initialization completed. Pre-loaded {} games.",
        game_count
    );
    Ok(progress_steps)
}

#[command]
async fn check_game_in_library(app_id: String) -> Result<bool, String> {
    let steam_config_path = find_steam_config_path().map_err(|e| e.to_string())?;
    let stplugin_dir = steam_config_path.join("stplug-in");

    if !stplugin_dir.exists() {
        return Ok(false);
    }

    let lua_file_path = stplugin_dir.join(format!("{}.lua", app_id));
    Ok(lua_file_path.exists())
}

#[command]
async fn get_library_games() -> Result<Vec<LibraryGame>, String> {
    // Display cache statistics
    GAME_CACHE.cache_stats();

    let steam_config_path = find_steam_config_path().map_err(|e| e.to_string())?;
    let stplugin_dir = steam_config_path.join("stplug-in");

    if !stplugin_dir.exists() {
        return Ok(Vec::new()); // Return empty list if directory doesn't exist
    }

    let mut app_ids = Vec::new();

    // Collect all app IDs first (only numeric ones)
    if let Ok(entries) = fs::read_dir(stplugin_dir) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("lua") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    // Only process numeric app IDs (skip files like "Steamtools.lua")
                    if stem.chars().all(|c| c.is_ascii_digit()) {
                        app_ids.push(stem.to_string());
                    } else {
                        println!("Skipping non-numeric app_id: {}", stem);
                    }
                }
            }
        }
    }

    // Only log in debug mode
    #[cfg(debug_assertions)]
    println!("Found {} games in library", app_ids.len());

    // Instant loading strategy: prioritize cached data for immediate UX
    let mut games = Vec::new();
    let mut uncached_ids = Vec::new();

    println!("📚 Loading library: {} total games", app_ids.len());

    // First pass: load all cached games immediately (instant UX)
    for app_id in &app_ids {
        if let Some(cached_name) = GAME_CACHE.get_game_name(app_id).await {
            games.push(LibraryGame {
                app_id: app_id.clone(),
                name: cached_name,
                header_image: header_image_for(app_id),
            });
        } else {
            uncached_ids.push(app_id.clone());
        }
    }

    let cached_count = games.len();
    println!(
        "⚡ Instant load: {} cached games, {} need fetching",
        cached_count,
        uncached_ids.len()
    );

    // If circuit breaker is open, just return cached games + placeholders
    let is_circuit_open = { *GAME_CACHE.circuit_breaker_open.lock().unwrap() };

    if is_circuit_open {
        println!("⚠️ Circuit breaker is open - returning cached games only");
        for app_id in uncached_ids {
            games.push(LibraryGame {
                app_id: app_id.clone(),
                name: format!("Game {}", app_id),
                header_image: header_image_for(&app_id),
            });
        }
    } else if !uncached_ids.is_empty() {
        // Smart strategy: fetch more critical games when cache is invalid
        let critical_count = if cached_count > 15 {
            // If we already have many cached games, fetch fewer immediately
            uncached_ids.len().min(8)
        } else if cached_count > 5 {
            // Medium cache coverage, fetch moderate amount
            uncached_ids.len().min(15)
        } else {
            // Low cache coverage, fetch more to reduce "Loading..." placeholders
            uncached_ids.len().min(20)
        };

        let (critical_ids, background_ids): (Vec<_>, Vec<_>) = uncached_ids
            .into_iter()
            .enumerate()
            .partition(|(i, _)| *i < critical_count);

        let critical_ids: Vec<_> = critical_ids.into_iter().map(|(_, id)| id).collect();
        let background_ids: Vec<_> = background_ids.into_iter().map(|(_, id)| id).collect();

        // Fetch critical games immediately
        if !critical_ids.is_empty() {
            println!(
                "🔥 Fetching {} critical games immediately...",
                critical_ids.len()
            );

            let batch_size = if critical_ids.len() > 8 { 2 } else { 3 };

            let mut fetched_games: Vec<LibraryGame> = stream::iter(critical_ids)
                .map(|app_id: String| async move {
                    let name = fetch_game_name_simple(&app_id)
                        .await
                        .unwrap_or_else(|| format!("Game {}", app_id));
                    
                    LibraryGame {
                        app_id: app_id.clone(),
                        name,
                        header_image: header_image_for(&app_id),
                    }
                })
                .buffer_unordered(batch_size)
                .collect()
                .await;

            games.append(&mut fetched_games);
            println!("✅ Loaded {} critical games", fetched_games.len());
        }

        // Queue background games for later processing
        if !background_ids.is_empty() {
            let background_count = background_ids.len();

            for app_id in &background_ids {
                GAME_CACHE.queue_for_refresh(app_id.clone());
            }

            // Add placeholders for background games (will be updated later)
            for app_id in &background_ids {
                games.push(LibraryGame {
                    app_id: app_id.clone(),
                    name: format!("Loading... ({})", &app_id[..6.min(app_id.len())]),
                    header_image: header_image_for(app_id),
                });
            }

            // Trigger background processing
            tokio::spawn(async {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                GAME_CACHE.process_queue_batch().await;
            });

            println!(
                "🔄 Queued {} games for background loading",
                background_count
            );
        }
    }

    // Sort games by name alphabetically
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    // Final summary
    let total_games = games.len();
    let loading_placeholders = games
        .iter()
        .filter(|g| g.name.starts_with("Loading..."))
        .count();

    if loading_placeholders > 0 {
        println!(
            "🎯 Library ready: {} games ({} instant, {} loading in background)",
            total_games,
            total_games - loading_placeholders,
            loading_placeholders
        );
    } else {
        println!("🎯 Library complete: {} games fully loaded", total_games);
    }

    // Final cache stats
    GAME_CACHE.cache_stats();
    Ok(games)
}

fn header_image_for(app_id: &str) -> String {
    format!(
        "https://cdn.akamai.steamstatic.com/steam/apps/{}/header.jpg",
        app_id
    )
}

// Helper function to extract AppID from Steam URLs
fn extract_appid_from_url(input: &str) -> Option<String> {
    // Support various Steam URL formats:
    // https://store.steampowered.com/app/1086940/Baldurs_Gate_3/
    // https://store.steampowered.com/app/1086940/
    // store.steampowered.com/app/1086940/
    // steam://store/1086940
    // steamcommunity.com/app/1086940

    let input = input.trim();

    // Remove protocol if present
    let url_part = if input.starts_with("http://") {
        &input[7..]
    } else if input.starts_with("https://") {
        &input[8..]
    } else if input.starts_with("steam://") {
        &input[8..]
    } else {
        input
    };

    // Steam store URL patterns
    if url_part.contains("store.steampowered.com/app/") {
        if let Some(start) = url_part.find("/app/") {
            let after_app = &url_part[start + 5..]; // Skip "/app/"
            if let Some(end) = after_app.find('/') {
                let app_id = &after_app[..end];
                if app_id.chars().all(|c| c.is_ascii_digit()) {
                    return Some(app_id.to_string());
                }
            } else {
                // No trailing slash, take everything after /app/
                if after_app.chars().all(|c| c.is_ascii_digit()) {
                    return Some(after_app.to_string());
                }
            }
        }
    }

    // Steam community URL patterns
    if url_part.contains("steamcommunity.com/app/") {
        if let Some(start) = url_part.find("/app/") {
            let after_app = &url_part[start + 5..]; // Skip "/app/"
            if let Some(end) = after_app.find('/') {
                let app_id = &after_app[..end];
                if app_id.chars().all(|c| c.is_ascii_digit()) {
                    return Some(app_id.to_string());
                }
            } else {
                if after_app.chars().all(|c| c.is_ascii_digit()) {
                    return Some(after_app.to_string());
                }
            }
        }
    }

    // Steam protocol URL (steam://store/1086940)
    if url_part.starts_with("store/") {
        let after_store = &url_part[6..]; // Skip "store/"
        if let Some(end) = after_store.find('/') {
            let app_id = &after_store[..end];
            if app_id.chars().all(|c| c.is_ascii_digit()) {
                return Some(app_id.to_string());
            }
        } else {
            if after_store.chars().all(|c| c.is_ascii_digit()) {
                return Some(after_store.to_string());
            }
        }
    }

    // Regex fallback for any URL containing /app/NUMBER pattern
    if let Ok(re) = Regex::new(r"/app/(\d+)") {
        if let Some(captures) = re.captures(input) {
            if let Some(app_id) = captures.get(1) {
                return Some(app_id.as_str().to_string());
            }
        }
    }

    None
}

async fn fetch_game_details_background(app_id: &str) -> Option<GameDetail> {
    // Background refresh for game details (no user-facing errors)
    let url = format!(
        "https://store.steampowered.com/api/appdetails?appids={}",
        app_id
    );

    match HTTP_CLIENT.get(&url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<serde_json::Value>().await {
                    Ok(v) => {
                        if let Some(data) = v.get(app_id).and_then(|x| x.get("data")) {
                            // Parse game detail (simplified for background refresh)
                            let name = data
                                .get("name")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string();
                            let header_image = header_image_for(app_id);
                            let banner_image = data
                                .get("background")
                                .and_then(|x| x.as_str())
                                .or_else(|| data.get("background_raw").and_then(|x| x.as_str()))
                                .unwrap_or(&header_image)
                                .to_string();

                            let game_detail = GameDetail {
                                app_id: app_id.to_string(),
                                name,
                                header_image,
                                banner_image,
                                detailed_description: data
                                    .get("detailed_description")
                                    .and_then(|x| x.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                release_date: data
                                    .get("release_date")
                                    .and_then(|x| x.get("date"))
                                    .and_then(|x| x.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                publisher: data
                                    .get("publishers")
                                    .and_then(|x| x.get(0))
                                    .and_then(|x| x.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                trailer: data
                                    .get("movies")
                                    .and_then(|arr| arr.get(0))
                                    .and_then(|item| item.get("mp4"))
                                    .and_then(|mp4| mp4.get("max"))
                                    .and_then(|x| x.as_str())
                                    .map(|s| s.to_string()),
                                screenshots: Vec::new(), // Skip screenshots for background refresh
                                sysreq_min: Vec::new(),
                                sysreq_rec: Vec::new(),
                                pc_requirements: None,
                                dlc: Vec::new(),
                                drm_notice: data
                                    .get("drm_notice")
                                    .and_then(|x| x.as_str())
                                    .map(|s| s.to_string()),
                            };

                            // Cache the refreshed data
                            GAME_CACHE.set_game_details(app_id.to_string(), game_detail.clone());
                            return Some(game_detail);
                        }
                    }
                    Err(_) => {}
                }
            }
        }
        Err(_) => {}
    }
    None
}

async fn fetch_game_name_simple(app_id: &str) -> Option<String> {
    // Check cache first (including stale-while-revalidate)
    if let Some(cached_name) = GAME_CACHE.get_game_name(app_id).await {
        return Some(cached_name);
    }

    // Check circuit breaker
    let is_circuit_open = { *GAME_CACHE.circuit_breaker_open.lock().unwrap() };

    if is_circuit_open {
        // Return placeholder when circuit is open
        return Some(format!("Game {}", app_id));
    }

    // Get or create a request lock for this app_id to prevent duplicate requests
    let request_lock = GAME_CACHE.get_or_create_request_lock(app_id).await;
    let _guard = request_lock.lock().await;

    // Check cache again after acquiring lock (another request might have completed)
    if let Some(cached_name) = GAME_CACHE.get_game_name(app_id).await {
        GAME_CACHE.remove_request_lock(app_id);
        return Some(cached_name);
    }

    // Throttle request to avoid rate limiting
    GAME_CACHE.throttle_request().await;

    let url = format!(
        "https://store.steampowered.com/api/appdetails?appids={}",
        app_id
    );
    // Minimal logging for game name fetch
    #[cfg(debug_assertions)]
    println!("Fetching game name: {}", app_id);

    let result = async {
        match HTTP_CLIENT.get(url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    if resp.status().as_u16() == 429 {
                        println!("Rate limited by Steam API (429)");
                        GAME_CACHE.record_error();
                    }
                    return None;
                }

                match resp.json::<serde_json::Value>().await {
                    Ok(v) => {
                        if let Some(data) = v.get(app_id).and_then(|x| x.get("data")) {
                            if let Some(name) = data.get("name").and_then(|x| x.as_str()) {
                                let name = name.to_string();
                                // Cache the result
                                GAME_CACHE.set_game_name(app_id.to_string(), name.clone());
                                // Reset error count on success
                                GAME_CACHE.reset_error_count();
                                return Some(name);
                            }
                        }
                        None
                    }
                    Err(e) => {
                        println!("Failed to parse JSON: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                println!("Request failed: {}", e);
                GAME_CACHE.record_error();
                None
            }
        }
    }
    .await;

    // Clean up the request lock
    GAME_CACHE.remove_request_lock(app_id);

    result
}

#[command]
async fn get_game_details(app_id: String) -> Result<GameDetail, String> {
    // Check cache first
    if let Some(cached_details) = GAME_CACHE.get_game_details(&app_id).await {
        return Ok(cached_details);
    }

    // Throttle request to avoid rate limiting
    GAME_CACHE.throttle_request().await;

    let url = format!(
        "https://store.steampowered.com/api/appdetails?appids={}",
        app_id
    );
    // Only log in debug mode and when not cached
    #[cfg(debug_assertions)]
    println!("Fetching game details from Steam API: {}", app_id);

    let resp = match HTTP_CLIENT.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            GAME_CACHE.record_error();
            return Err(format!("Request failed: {}", e));
        }
    };

    if !resp.status().is_success() {
        if resp.status().as_u16() == 429 {
            GAME_CACHE.record_error();
            return Err(
                "Rate limited by Steam API (429). Please wait before trying again.".to_string(),
            );
        }
        return Err(format!("status {}", resp.status()));
    }

    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    // Only log if there's an issue, not the full response
    #[cfg(debug_assertions)]
    if let Some(app_data) = v.get(&app_id) {
        if let Some(success) = app_data.get("success").and_then(|s| s.as_bool()) {
            if !success {
                println!("Steam API returned success=false for app ID {}", app_id);
            }
        }
    }

    // Check if the app exists and was successful
    let app_data = v
        .get(&app_id)
        .ok_or_else(|| format!("App ID {} not found in Steam API response", app_id))?;

    if let Some(success) = app_data.get("success").and_then(|s| s.as_bool()) {
        if !success {
            return Err(format!("Steam API returned success=false for app ID {} (game might not exist or be private)", app_id));
        }
    }

    let data = app_data.get("data").ok_or_else(|| {
        format!(
            "No data field for app ID {} (app might not exist or be private)",
            app_id
        )
    })?;

    // Reset error count on successful request
    GAME_CACHE.reset_error_count();

    let name = data
        .get("name")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    // Always use our consistent header image format instead of Steam API's variable quality images
    let header_image = header_image_for(&app_id);

    // Use background image for banner (higher resolution)
    let banner_image = data
        .get("background")
        .and_then(|x| x.as_str())
        .or_else(|| data.get("background_raw").and_then(|x| x.as_str()))
        .unwrap_or(&header_image)
        .to_string();

    let detailed_description = data
        .get("detailed_description")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let release_date = data
        .get("release_date")
        .and_then(|x| x.get("date"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let publisher = data
        .get("publishers")
        .and_then(|x| x.get(0))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();

    let trailer = data
        .get("movies")
        .and_then(|arr| arr.get(0))
        .and_then(|item| item.get("mp4"))
        .and_then(|mp4| mp4.get("max"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let mut screenshots = Vec::new();
    if let Some(arr) = data.get("screenshots").and_then(|x| x.as_array()) {
        for s in arr.iter().take(6) {
            // Use full resolution image instead of thumbnail
            if let Some(url) = s.get("path_full").and_then(|x| x.as_str()) {
                screenshots.push(url.to_string());
            }
        }
    }

    // Parse system requirements
    let mut sysreq_min = Vec::new();
    let mut sysreq_rec = Vec::new();
    let mut pc_requirements = None;

    if let Some(pc_req) = data.get("pc_requirements") {
        let minimum = pc_req
            .get("minimum")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());
        let recommended = pc_req
            .get("recommended")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());

        if minimum.is_some() || recommended.is_some() {
            pc_requirements = Some(PcRequirements {
                minimum: minimum.clone(),
                recommended: recommended.clone(),
            });
        }

        if let Some(min_str) = minimum.as_ref() {
            sysreq_min = parse_sysreq_html(min_str);
        }
        if let Some(rec_str) = recommended.as_ref() {
            sysreq_rec = parse_sysreq_html(rec_str);
        }
    }

    // DLC will be loaded separately when needed (lazy loading)
    let dlc = Vec::new();

    // Extract DRM notice
    let drm_notice = data
        .get("drm_notice")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());

    let game_detail = GameDetail {
        app_id: app_id.clone(),
        name,
        header_image,
        banner_image,
        detailed_description,
        release_date,
        publisher,
        trailer,
        screenshots,
        sysreq_min,
        sysreq_rec,
        pc_requirements,
        dlc,
        drm_notice,
    };

    // Cache the result
    GAME_CACHE.set_game_details(app_id, game_detail.clone());

    Ok(game_detail)
}

fn parse_sysreq_html(html: &str) -> Vec<(String, String)> {
    let lower = html
        .replace("<br>", "\n")
        .replace("<strong>", "")
        .replace("</strong>", "");
    let mut out = Vec::new();
    for line in lower.lines() {
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() == 2 {
            let key = parts[0].trim();
            let val = parts[1].trim();
            if !key.is_empty() && !val.is_empty() {
                out.push((key.to_string(), val.to_string()));
            }
        }
    }
    out
}

async fn process_and_install_to_steam(
    zip_bytes: &[u8],
    app_id: &str,
    _game_name: &str,
) -> Result<String, anyhow::Error> {
    println!("Processing ZIP and installing to Steam...");

    // Create temporary directory
    let temp_dir = TempDir::new()?;
    println!("Created temporary directory: {:?}", temp_dir.path());

    // Extract ZIP to temporary directory
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = temp_dir.path().join(file.name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    println!("Extracted {} files to temporary directory", archive.len());

    // Find Steam config directory
    let steam_config_path = find_steam_config_path()?;
    println!("Found Steam config at: {:?}", steam_config_path);

    // Define target directories
    let stplugin_dir = steam_config_path.join("stplug-in");
    let depotcache_dir = steam_config_path.join("depotcache");
    let statsexport_dir = steam_config_path.join("StatsExport");

    // Create target directories if they don't exist
    fs::create_dir_all(&stplugin_dir)?;
    fs::create_dir_all(&depotcache_dir)?;
    fs::create_dir_all(&statsexport_dir)?;

    // Count files moved
    let mut lua_count = 0;
    let mut manifest_count = 0;
    let mut bin_count = 0;
    let mut manifest_map: HashMap<String, String> = HashMap::new();

    // Walk through all files and process them
    for entry in WalkDir::new(temp_dir.path())
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_file() {
            let path = entry.path();
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();

            if let Some(ext) = path.extension() {
                match ext.to_str().unwrap_or("") {
                    "lua" => {
                        let target = stplugin_dir.join(path.file_name().unwrap_or_default());
                        fs::copy(path, &target)?;
                        lua_count += 1;
                        println!("Moved LUA file to stplug-in: {}", file_name);
                    }
                    "bin" => {
                        let target = statsexport_dir.join(path.file_name().unwrap_or_default());
                        fs::copy(path, &target)?;
                        bin_count += 1;
                        println!("Moved BIN file to StatsExport: {}", file_name);
                    }
                    "manifest" => {
                        let target = depotcache_dir.join(path.file_name().unwrap_or_default());
                        fs::copy(path, &target)?;
                        manifest_count += 1;
                        println!("Moved manifest file to depotcache: {}", file_name);

                        // Extract depot ID and manifest ID for LUA updates
                        let re = Regex::new(r"(\d+)_(\d+)\.manifest")?;
                        if let Some(caps) = re.captures(&file_name) {
                            let depot_id = caps.get(1).unwrap().as_str().to_string();
                            let manifest_id = caps.get(2).unwrap().as_str().to_string();
                            manifest_map.insert(depot_id, manifest_id);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Update LUA files with new manifest IDs if we have any
    if !manifest_map.is_empty() {
        update_lua_files(&stplugin_dir, app_id, &manifest_map)?;
    }

    // Cleanup happens automatically when temp_dir is dropped
    println!("Installation complete - temporary files cleaned up");

    Ok(format!(
        "Files installed: {} LUA, {} manifests, {} BIN files",
        lua_count, manifest_count, bin_count
    ))
}

fn find_steam_config_path() -> Result<PathBuf, anyhow::Error> {
    // Check common Windows Steam paths
    let common_paths = [
        "C:\\Program Files (x86)\\Steam\\config",
        "C:\\Program Files\\Steam\\config",
    ];

    for path in common_paths.iter() {
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }

    // Try registry lookup (Windows only)
    #[cfg(target_os = "windows")]
    {
        use winreg::{enums::*, RegKey};

        if let Ok(hkcu) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
            if let Ok(steam_path_str) = hkcu.get_value::<String, _>("SteamPath") {
                let config_path = PathBuf::from(steam_path_str).join("config");
                if config_path.exists() {
                    return Ok(config_path);
                }
            }
        }
    }

    Err(anyhow::anyhow!(
        "Steam config directory not found. Please make sure Steam is installed."
    ))
}

fn update_lua_files(
    stplugin_dir: &Path,
    app_id: &str,
    manifest_map: &HashMap<String, String>,
) -> Result<(), anyhow::Error> {
    // Find LUA file for this app ID
    if let Some(lua_file) = find_lua_file_for_appid(stplugin_dir, app_id)? {
        println!("Updating LUA file: {:?}", lua_file);

        let original_content = fs::read_to_string(&lua_file)?;
        let mut updated_content = original_content.clone();
        let mut updated_count = 0;

        // Update existing manifest IDs
        let re_replace = Regex::new(r#"setManifestid\s*\(\s*(\d+)\s*,\s*"(\d+)"\s*,\s*0\s*\)"#)?;
        updated_content = re_replace
            .replace_all(&updated_content, |caps: &regex::Captures| {
                let depot_id = caps.get(1).unwrap().as_str();
                let old_manifest_id = caps.get(2).unwrap().as_str();

                if let Some(new_manifest_id) = manifest_map.get(depot_id) {
                    if new_manifest_id != old_manifest_id {
                        updated_count += 1;
                        return format!(r#"setManifestid({}, "{}", 0)"#, depot_id, new_manifest_id);
                    }
                }
                caps.get(0).unwrap().as_str().to_string()
            })
            .to_string();

        // Append new manifest IDs that weren't in the file
        let existing_depots: Vec<String> = re_replace
            .captures_iter(&original_content)
            .map(|cap| cap[1].to_string())
            .collect();

        let mut new_lines = Vec::new();
        for (depot_id, manifest_id) in manifest_map {
            if !existing_depots.contains(depot_id) {
                new_lines.push(format!(
                    r#"setManifestid({}, "{}", 0)"#,
                    depot_id, manifest_id
                ));
                updated_count += 1;
            }
        }

        if !new_lines.is_empty() {
            updated_content.push_str("\n-- Updated by Zenith --\n");
            updated_content.push_str(&new_lines.join("\n"));
            updated_content.push('\n');
        }

        if updated_count > 0 {
            fs::write(&lua_file, updated_content)?;
            println!("Updated {} manifest entries in LUA file", updated_count);
        }
    }

    Ok(())
}

fn find_lua_file_for_appid(
    stplugin_dir: &Path,
    app_id: &str,
) -> Result<Option<PathBuf>, anyhow::Error> {
    for entry in WalkDir::new(stplugin_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_file() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "lua" {
                    // Check if filename matches AppID (e.g., 413150.lua)
                    if let Some(stem) = path.file_stem() {
                        if stem.to_string_lossy() == app_id {
                            return Ok(Some(path.to_path_buf()));
                        }
                    }

                    // Check if file content contains addappid(AppID)
                    if let Ok(content) = fs::read_to_string(path) {
                        let re = Regex::new(&format!(r"addappid\s*\(\s*({})\s*\)", app_id))?;
                        if re.is_match(&content) {
                            return Ok(Some(path.to_path_buf()));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

#[command]
fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to Zenith!", name)
}

// Helper function to find Steam executable path
fn find_steam_executable_path() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        // Try to get Steam path from registry
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(steam_key) = hklm.open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam") {
            if let Ok(install_path) = steam_key.get_value::<String, _>("InstallPath") {
                let steam_exe = format!("{}\\steam.exe", install_path);
                if std::path::Path::new(&steam_exe).exists() {
                    return Ok(steam_exe);
                }
            }
        }

        // Try 32-bit registry path
        if let Ok(steam_key) = hklm.open_subkey("SOFTWARE\\Valve\\Steam") {
            if let Ok(install_path) = steam_key.get_value::<String, _>("InstallPath") {
                let steam_exe = format!("{}\\steam.exe", install_path);
                if std::path::Path::new(&steam_exe).exists() {
                    return Ok(steam_exe);
                }
            }
        }

        // Fallback to common installation paths
        let common_paths = vec![
            "C:\\Program Files (x86)\\Steam\\steam.exe",
            "C:\\Program Files\\Steam\\steam.exe",
        ];

        for path in common_paths {
            if std::path::Path::new(path).exists() {
                return Ok(path.to_string());
            }
        }

        Err("Steam installation not found".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        // For Linux/macOS, assume steam is in PATH
        Ok("steam".to_string())
    }
}

#[command]
async fn restart_steam() -> Result<String, String> {
    println!("Attempting to restart Steam...");

    #[cfg(target_os = "windows")]
    {
        // First, terminate the Steam process
        let kill_result = Command::new("taskkill")
            .args(&["/F", "/IM", "steam.exe"])
            .output();

        match kill_result {
            Ok(output) => {
                if output.status.success() {
                    println!("Steam process terminated successfully");
                } else {
                    println!("Steam process might not be running");
                }
            }
            Err(e) => {
                println!("Failed to terminate Steam: {}", e);
            }
        }

        // Wait a moment for the process to fully terminate
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // Find and restart Steam
        match find_steam_executable_path() {
            Ok(steam_path) => match Command::new(&steam_path).spawn() {
                Ok(_) => {
                    println!("Steam restarted successfully");
                    Ok("Steam has been restarted successfully".to_string())
                }
                Err(e) => {
                    let error_msg = format!("Failed to restart Steam: {}", e);
                    println!("{}", error_msg);
                    Err(error_msg)
                }
            },
            Err(e) => {
                let error_msg = format!("Steam executable not found: {}", e);
                println!("{}", error_msg);
                Err(error_msg)
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // For Linux/macOS, try to restart Steam
        match Command::new("pkill").arg("steam").output() {
            Ok(_) => println!("Steam process terminated"),
            Err(_) => println!("Steam might not be running"),
        }

        tokio::time::sleep(Duration::from_millis(1000)).await;

        match Command::new("steam").spawn() {
            Ok(_) => {
                println!("Steam restarted successfully");
                Ok("Steam has been restarted successfully".to_string())
            }
            Err(e) => {
                let error_msg = format!("Failed to restart Steam: {}", e);
                println!("{}", error_msg);
                Err(error_msg)
            }
        }
    }
}

#[command]
async fn check_steam_status() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        match Command::new("tasklist")
            .args(&["/FI", "IMAGENAME eq steam.exe"])
            .output()
        {
            Ok(output) => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                let is_running = output_str.contains("steam.exe");
                println!(
                    "Steam status check: {}",
                    if is_running { "Running" } else { "Not running" }
                );
                Ok(is_running)
            }
            Err(e) => {
                println!("Failed to check Steam status: {}", e);
                Err(format!("Failed to check Steam status: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        match Command::new("pgrep").arg("steam").output() {
            Ok(output) => {
                let is_running = !output.stdout.is_empty();
                println!(
                    "Steam status check: {}",
                    if is_running { "Running" } else { "Not running" }
                );
                Ok(is_running)
            }
            Err(e) => {
                println!("Failed to check Steam status: {}", e);
                Err(format!("Failed to check Steam status: {}", e))
            }
        }
    }
}

#[command]
async fn get_game_dlc_list(app_id: String) -> Result<Vec<String>, String> {
    println!("Fetching DLC list for game: {}", app_id);

    // Check if we have cached game details with DLC
    if let Some(cached_details) = GAME_CACHE.get_game_details(&app_id).await {
        if !cached_details.dlc.is_empty() {
            println!(
                "Found {} cached DLCs for game {}",
                cached_details.dlc.len(),
                app_id
            );
            return Ok(cached_details.dlc);
        }
    }

    // Fetch DLC data from Steam API
    let url = format!(
        "https://store.steampowered.com/api/appdetails?appids={}",
        app_id
    );
    println!("Fetching DLC data from Steam API for: {}", app_id);

    let resp = HTTP_CLIENT
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Steam API returned status {}", resp.status()));
    }

    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let data = v
        .get(&app_id)
        .and_then(|x| x.get("data"))
        .ok_or("no data")?;

    // Extract DLC information
    let dlc = if let Some(dlc_data) = data.get("dlc") {
        match dlc_data {
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_u64())
                .map(|id| id.to_string())
                .collect(),
            serde_json::Value::Object(obj) => obj
                .values()
                .filter_map(|v| v.as_u64())
                .map(|id| id.to_string())
                .collect(),
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };

    if !dlc.is_empty() {
        println!("Found {} DLCs for game {}", dlc.len(), app_id);

        // Update cached game details with DLC info
        if let Some(mut cached_details) = GAME_CACHE.get_game_details(&app_id).await {
            cached_details.dlc = dlc.clone();
            GAME_CACHE.set_game_details(app_id.clone(), cached_details);
        }
    }

    Ok(dlc)
}

#[command]
async fn get_batch_game_details(app_ids: Vec<String>) -> Result<Vec<GameDetail>, String> {
    println!("Fetching batch details for {} DLCs", app_ids.len());
    let mut details_list = Vec::new();
    let mut cache_hits = 0;
    let mut api_calls = 0;

    // Process in smaller batches to avoid overwhelming the API
    for chunk in app_ids.chunks(5) {
        let mut batch_futures = Vec::new();

        for app_id in chunk {
            // Check cache first to count hits
            if GAME_CACHE.get_game_details(app_id).await.is_some() {
                cache_hits += 1;
            } else {
                api_calls += 1;
            }
            batch_futures.push(get_game_details(app_id.clone()));
        }

        // Wait for all in this batch
        for future in batch_futures {
            match future.await {
                Ok(details) => details_list.push(details),
                Err(e) => println!("Could not fetch details for AppID: {}", e),
            }
        }

        // Small delay between batches
        if details_list.len() < app_ids.len() {
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }

    println!(
        "Batch DLC fetch completed: {} cache hits, {} API calls, {} total results",
        cache_hits,
        api_calls,
        details_list.len()
    );
    Ok(details_list)
}

#[command]
async fn get_dlcs_in_lua(app_id: String) -> Result<Vec<String>, String> {
    let steam_config_path = find_steam_config_path().map_err(|e| e.to_string())?;
    let stplugin_dir = steam_config_path.join("stplug-in");
    let lua_file_path = stplugin_dir.join(format!("{}.lua", app_id));

    if !lua_file_path.exists() {
        return Ok(Vec::new()); // No lua file means no DLCs installed
    }

    let content = fs::read_to_string(&lua_file_path).map_err(|e| e.to_string())?;

    // Parse addappid() calls from lua file
    let re = Regex::new(r"addappid\s*\(\s*(\d+)\s*\)").unwrap();
    let installed_dlcs: Vec<String> = re
        .captures_iter(&content)
        .map(|cap| cap[1].to_string())
        .filter(|id| *id != app_id) // Exclude the main game's ID
        .collect();

    println!(
        "Found {} installed DLCs for game {}",
        installed_dlcs.len(),
        app_id
    );
    Ok(installed_dlcs)
}

#[command]
async fn sync_dlcs_in_lua(
    main_app_id: String,
    dlc_ids_to_set: Vec<String>,
    added_count: Option<usize>,
    removed_count: Option<usize>,
) -> Result<String, String> {
    let steam_config_path = find_steam_config_path().map_err(|e| e.to_string())?;
    let stplugin_dir = steam_config_path.join("stplug-in");
    let lua_file_path = stplugin_dir.join(format!("{}.lua", main_app_id));

    // Read existing content or create new
    let original_content = if lua_file_path.exists() {
        fs::read_to_string(&lua_file_path).map_err(|e| e.to_string())?
    } else {
        // Create new lua file if it doesn't exist
        String::new()
    };

    // Remove existing DLC entries but keep the main game and other content
    let addappid_re = Regex::new(r"addappid\s*\(\s*(\d+)\s*\)").unwrap();
    let filtered_lines: Vec<&str> = original_content
        .lines()
        .filter(|line| {
            if let Some(caps) = addappid_re.captures(line) {
                if let Some(id_str) = caps.get(1) {
                    // Keep only the main game ID, remove all DLC IDs
                    return id_str.as_str() == main_app_id;
                }
            }
            // Keep non-addappid lines
            true
        })
        .collect();

    let mut new_content = filtered_lines.join("\n");

    // Add main game if not present
    if !new_content.contains(&format!("addappid({})", main_app_id)) {
        if !new_content.is_empty() && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push_str(&format!("addappid({})\n", main_app_id));
    }

    // Add selected DLCs
    if !dlc_ids_to_set.is_empty() {
        if !new_content.is_empty() && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push_str("\n-- DLCs managed by Zenith --\n");
        for dlc_id in &dlc_ids_to_set {
            new_content.push_str(&format!("addappid({})\n", dlc_id));
        }
    }

    // Ensure directory exists
    if let Some(parent) = lua_file_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Write back to file
    fs::write(&lua_file_path, new_content).map_err(|e| e.to_string())?;

    // Generate appropriate message based on actions
    let message = match (added_count.unwrap_or(0), removed_count.unwrap_or(0)) {
        (0, 0) => "No changes made to DLCs".to_string(),
        (added, 0) if added > 0 => format!(
            "Successfully unlocked {} DLC{}",
            added,
            if added == 1 { "" } else { "s" }
        ),
        (0, removed) if removed > 0 => format!(
            "Successfully removed {} DLC{}",
            removed,
            if removed == 1 { "" } else { "s" }
        ),
        (added, removed) => format!(
            "Successfully unlocked {} and removed {} DLC{}",
            added,
            removed,
            if added + removed == 1 { "" } else { "s" }
        ),
    };

    println!("DLC sync completed for game {}: {}", main_app_id, message);
    Ok(message)
}

#[command]
async fn clear_cache() -> Result<String, String> {
    GAME_CACHE.clear_all();
    Ok("Cache cleared successfully".to_string())
}

#[command]
async fn refresh_cache_background() -> Result<String, String> {
    // Trigger background cache refresh
    tokio::spawn(async {
        GAME_CACHE.process_queue_batch().await;
    });
    Ok("Background cache refresh started".to_string())
}

#[command]
async fn remove_game(app_id: String) -> Result<DownloadResult, String> {
    println!("Removing game with AppID: {}", app_id);

    let steam_config_path = find_steam_config_path().map_err(|e| e.to_string())?;

    // Define target directories
    let stplugin_dir = steam_config_path.join("stplug-in");
    let depotcache_dir = steam_config_path.join("depotcache");
    let statsexport_dir = steam_config_path.join("StatsExport");

    let mut removed_files = Vec::new();

    // Delete LUA file
    let lua_file = stplugin_dir.join(format!("{}.lua", app_id));
    if lua_file.exists() {
        if let Err(e) = fs::remove_file(&lua_file) {
            return Ok(DownloadResult {
                success: false,
                message: format!("Failed to delete LUA file: {}", e),
                file_path: None,
            });
        }
        removed_files.push("LUA");
    }

    // Delete manifest files (there might be multiple)
    if let Ok(entries) = fs::read_dir(&depotcache_dir) {
        for entry in entries.filter_map(Result::ok) {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            // Check if it's a manifest file for this app
            if file_name_str.contains(&app_id) && file_name_str.ends_with(".manifest") {
                if let Err(e) = fs::remove_file(entry.path()) {
                    println!(
                        "Warning: Failed to delete manifest file {}: {}",
                        file_name_str, e
                    );
                } else {
                    removed_files.push("manifest");
                }
            }
        }
    }

    // Delete BIN files
    if let Ok(entries) = fs::read_dir(&statsexport_dir) {
        for entry in entries.filter_map(Result::ok) {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            // Check if it's a BIN file for this app
            if file_name_str.contains(&app_id) && file_name_str.ends_with(".bin") {
                if let Err(e) = fs::remove_file(entry.path()) {
                    println!(
                        "Warning: Failed to delete BIN file {}: {}",
                        file_name_str, e
                    );
                } else {
                    removed_files.push("BIN");
                }
            }
        }
    }

    if removed_files.is_empty() {
        Ok(DownloadResult {
            success: false,
            message: "No game files found to remove".to_string(),
            file_path: None,
        })
    } else {
        Ok(DownloadResult {
            success: true,
            message: format!(
                "Removed {} files: {}",
                removed_files.len(),
                removed_files.join(", ")
            ),
            file_path: None,
        })
    }
}

// ====================== BYPASS FUNCTIONS ======================

#[command]
async fn check_bypass_availability(app_id: String) -> Result<BypassStatus, String> {
    println!("Checking bypass availability for AppID: {}", app_id);

    let bypass_url = format!("https://bypass.nzr.web.id/{}.zip", app_id);

    // HEAD request untuk cek file exists + size
    match DOWNLOAD_CLIENT.head(&bypass_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                // Check if bypass already installed
                let is_installed = check_bypass_installed(&app_id).await.unwrap_or(false);

                Ok(BypassStatus {
                    available: true,
                    installing: false,
                    installed: is_installed,
                })
            } else {
                Ok(BypassStatus {
                    available: false,
                    installing: false,
                    installed: false,
                })
            }
        }
        Err(e) => {
            println!("Error checking bypass availability: {}", e);
            Ok(BypassStatus {
                available: false,
                installing: false,
                installed: false,
            })
        }
    }
}

async fn check_bypass_installed(app_id: &str) -> Result<bool, String> {
    // Check if bypass files exist in game directory
    match find_steam_installation_path() {
        Ok(steam_path) => {
            match find_game_folder_from_acf(app_id, &steam_path).await {
                Some(game_folder) => {
                    let game_path = format!("{}/steamapps/common/{}", steam_path, game_folder);

                    // Check for bypass installation marker
                    let bypass_indicators = vec!["bypass_installed.txt"];

                    for indicator in bypass_indicators {
                        let indicator_path = format!("{}/{}", game_path, indicator);
                        if Path::new(&indicator_path).exists() {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                }
                None => Ok(false),
            }
        }
        Err(_) => Ok(false),
    }
}

#[command]
async fn install_bypass(app_id: String, window: tauri::Window) -> Result<BypassResult, String> {
    // Check if bypass is already installed
    let is_reinstall = check_bypass_installed(&app_id).await.unwrap_or(false);

    if is_reinstall {
        println!("🔄 Starting bypass REINSTALLATION for AppID: {}", app_id);
        println!("   (Previous installation detected - will overwrite)");
    } else {
        println!("🚀 Starting bypass installation for AppID: {}", app_id);
    }
    println!("================================");

    let emit_progress = |step: &str, progress: f64| {
        println!("📊 Progress: {:.1}% - {}", progress, step);
        let _ = window.emit(
            "bypass_progress",
            BypassProgress {
                step: step.to_string(),
                progress,
                app_id: app_id.clone(),
            },
        );
    };

    let action_word = if is_reinstall {
        "reinstallation"
    } else {
        "installation"
    };
    emit_progress(&format!("Initializing bypass {}...", action_word), 0.0);

    // Step 1: Detect Steam installation
    emit_progress("Detecting Steam installation...", 10.0);
    let steam_path = find_steam_installation_path()
        .map_err(|e| format!("Steam installation not found: {}", e))?;

    // Step 2: Validate game installation
    emit_progress("Validating game installation...", 20.0);
    let game_folder = find_game_folder_from_acf(&app_id, &steam_path)
        .await
        .ok_or_else(|| "Game not found in Steam library or not fully installed".to_string())?;
    println!("📁 Found game folder: {}", game_folder);

    let game_path = format!("{}/steamapps/common/{}", steam_path, game_folder);
    println!("🎯 Full game path: {}", game_path);

    if !Path::new(&game_path).exists() {
        let error_msg = format!("Game directory does not exist: {}", game_path);
        println!("❌ {}", error_msg);
        return Err(error_msg);
    }

    println!("✅ Game directory validated successfully");

    // Step 3: Check bypass availability
    emit_progress("Checking bypass availability...", 30.0);
    let bypass_url = format!("https://bypass.nzr.web.id/{}.zip", app_id);

    // Step 4: Download bypass
    emit_progress("Downloading bypass files...", 40.0);
    println!("🌐 Bypass URL: {}", bypass_url);

    let download_path = download_bypass_with_progress(&bypass_url, &window, &app_id)
        .await
        .map_err(|e| {
            println!("❌ Download completely failed: {}", e);
            format!("Failed to download bypass: {}", e)
        })?;

    // Step 5: Extract bypass
    emit_progress("Extracting bypass files...", 70.0);
    let extract_path = extract_bypass(&download_path)
        .await
        .map_err(|e| format!("Failed to extract bypass: {}", e))?;

    // Step 6: Install bypass files
    emit_progress("Installing bypass to game directory...", 85.0);
    install_bypass_files(&extract_path, &game_path)
        .await
        .map_err(|e| format!("Failed to install bypass: {}", e))?;

    // Step 7: Cleanup
    emit_progress("Finalizing installation...", 95.0);
    cleanup_temp_files(&download_path, &extract_path)?;
    println!("🧹 Cleaned up temporary files");

    // Mark bypass as installed
    let installed_marker = format!("{}/bypass_installed.txt", game_path);
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
    let marker_content = format!(
        "Bypass installed by Zenith\nAppID: {}\nInstalled: {}\nGame Path: {}",
        app_id, timestamp, game_path
    );
    let _ = fs::write(&installed_marker, marker_content);
    println!("📝 Created installation marker");

    let final_message = if is_reinstall {
        "Bypass reinstalled successfully!"
    } else {
        "Bypass installed successfully!"
    };

    emit_progress(final_message, 100.0);

    println!("🎉 Bypass {} completed successfully!", action_word);
    println!("🎯 Always showing launch popup - user can choose executable");
    println!("📁 Game directory: {}", game_path);
    println!("================================");

    // Always show launch popup and let user navigate to executable
    Ok(BypassResult {
        success: true,
        message: final_message.to_string(),
        should_launch: true,                   // Always true - show popup
        game_executable_path: Some(game_path), // Pass game directory path
    })
}

async fn find_game_folder_from_acf(app_id: &str, steam_path: &str) -> Option<String> {
    let steamapps_path = format!("{}/steamapps", steam_path);
    let acf_file = format!("{}/appmanifest_{}.acf", steamapps_path, app_id);

    if let Ok(content) = fs::read_to_string(&acf_file) {
        // Parse ACF untuk cari "installdir"
        for line in content.lines() {
            if line.contains("\"installdir\"") {
                if let Some(folder) = line.split('"').nth(3) {
                    return Some(folder.to_string());
                }
            }
        }
    }
    None
}

fn find_steam_installation_path() -> Result<String, String> {
    // Fixed Steam path as requested
    let steam_path = "C:\\Program Files (x86)\\Steam";

    if Path::new(&format!("{}/steam.exe", steam_path)).exists() {
        return Ok(steam_path.to_string());
    }

    // Fallback to registry lookup if fixed path doesn't work
    #[cfg(target_os = "windows")]
    {
        use winreg::{enums::*, RegKey};

        if let Ok(hklm) =
            RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam")
        {
            if let Ok(install_path) = hklm.get_value::<String, _>("InstallPath") {
                if Path::new(&format!("{}/steam.exe", install_path)).exists() {
                    return Ok(install_path);
                }
            }
        }

        if let Ok(hklm) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey("SOFTWARE\\Valve\\Steam") {
            if let Ok(install_path) = hklm.get_value::<String, _>("InstallPath") {
                if Path::new(&format!("{}/steam.exe", install_path)).exists() {
                    return Ok(install_path);
                }
            }
        }
    }

    // Additional fallback paths
    let fallback_paths = vec!["C:\\Program Files\\Steam", "D:\\Steam", "E:\\Steam"];

    for path in fallback_paths {
        if Path::new(&format!("{}/steam.exe", path)).exists() {
            return Ok(path.to_string());
        }
    }

    Err("Steam installation not found".to_string())
}

async fn download_bypass_with_progress(
    bypass_url: &str,
    window: &tauri::Window,
    app_id: &str,
) -> Result<String, String> {
    println!("📥 Starting download from: {}", bypass_url);

    // Try download with retry mechanism
    let mut last_error = String::new();

    for attempt in 1..=3 {
        println!("🔄 Download attempt {} of 3", attempt);

        match download_bypass_attempt(bypass_url, window, app_id, attempt).await {
            Ok(path) => {
                println!("✅ Download successful on attempt {}", attempt);
                return Ok(path);
            }
            Err(e) => {
                last_error = e.clone();
                println!("❌ Attempt {} failed: {}", attempt, e);

                if attempt < 3 {
                    println!("⏳ Waiting 3 seconds before retry...");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
    }

    Err(format!(
        "Download failed after 3 attempts. Last error: {}",
        last_error
    ))
}

async fn download_bypass_attempt(
    bypass_url: &str,
    window: &tauri::Window,
    app_id: &str,
    attempt: u32,
) -> Result<String, String> {
    let mut response = DOWNLOAD_CLIENT
        .get(bypass_url)
        .send()
        .await
        .map_err(|e| format!("Failed to start download: {}", e))?;

    if !response.status().is_success() {
        let error_msg = format!("Download failed with status: {}", response.status());
        return Err(error_msg);
    }

    let total_size = response.content_length().unwrap_or(0);
    println!(
        "📦 Download size: {:.2} MB",
        total_size as f64 / 1_048_576.0
    );

    let temp_dir = std::env::temp_dir();
    let download_path = temp_dir.join(format!(
        "bypass_{}_{}_{}.zip",
        app_id,
        attempt,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    ));

    let mut file =
        File::create(&download_path).map_err(|e| format!("Failed to create temp file: {}", e))?;

    let mut downloaded = 0u64;
    let mut last_progress_time = std::time::Instant::now();

    println!("📊 Starting download stream (attempt {})...", attempt);

    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;

        // Update progress every 500ms to avoid spam
        if last_progress_time.elapsed() >= Duration::from_millis(500) {
            if total_size > 0 {
                let progress = (downloaded as f64 / total_size as f64) * 30.0 + 40.0;
                let speed_mbps =
                    (downloaded as f64 / 1_048_576.0) / last_progress_time.elapsed().as_secs_f64();

                let _ = window.emit(
                    "bypass_progress",
                    BypassProgress {
                        step: format!(
                            "Downloading... {:.1} MB / {:.1} MB ({:.1} MB/s)",
                            downloaded as f64 / 1_048_576.0,
                            total_size as f64 / 1_048_576.0,
                            speed_mbps
                        ),
                        progress,
                        app_id: app_id.to_string(),
                    },
                );
            }
            last_progress_time = std::time::Instant::now();
        }
    }

    println!("✅ Download completed: {}", download_path.display());
    Ok(download_path.to_string_lossy().to_string())
}

async fn extract_bypass(zip_path: &str) -> Result<String, String> {
    println!("📂 Extracting bypass files from: {}", zip_path);

    let extract_dir = std::env::temp_dir().join(format!(
        "bypass_extract_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    ));

    fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;
    println!("📁 Extract directory: {}", extract_dir.display());

    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;

    println!("📋 Archive contains {} files", archive.len());

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let outpath = match file.enclosed_name() {
            Some(path) => extract_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut outfile = File::create(&outpath).map_err(|e| e.to_string())?;
            io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
        }
    }

    println!("✅ Extraction completed to: {}", extract_dir.display());
    Ok(extract_dir.to_string_lossy().to_string())
}

async fn install_bypass_files(extract_path: &str, game_path: &str) -> Result<(), String> {
    println!("🔧 Installing bypass files");
    println!("   Source: {}", extract_path);
    println!("   Target: {}", game_path);

    // Find the actual bypass files - they might be in a subfolder
    let bypass_source = find_bypass_files_directory(extract_path)?;
    println!("🎯 Bypass files found in: {}", bypass_source);

    // Copy files directly to game directory (flatten structure)
    copy_bypass_files_flat(&bypass_source, game_path)?;
    println!("✅ Bypass files installed successfully");

    Ok(())
}

// Find where the actual bypass files are located (might be in subfolder)
fn find_bypass_files_directory(extract_path: &str) -> Result<String, String> {
    let extract_dir = Path::new(extract_path);

    // First, check if there are executable files directly in extract path
    let mut has_exe_files = false;
    let mut has_dll_files = false;

    if let Ok(entries) = fs::read_dir(extract_dir) {
        for entry in entries.filter_map(Result::ok) {
            if let Some(ext) = entry.path().extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if ext_str == "exe" {
                    has_exe_files = true;
                }
                if ext_str == "dll" {
                    has_dll_files = true;
                }
            }
        }
    }

    // If we have bypass files directly in extract path, use it
    if has_exe_files || has_dll_files {
        println!("📁 Bypass files found directly in extract directory");
        return Ok(extract_path.to_string());
    }

    // Otherwise, look for subfolder containing bypass files
    println!("🔍 Searching for bypass files in subfolders...");

    for entry in WalkDir::new(extract_dir).max_depth(2) {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_dir() && path != extract_dir {
                // Check if this directory contains bypass files
                let mut exe_count = 0;
                let mut dll_count = 0;

                if let Ok(sub_entries) = fs::read_dir(path) {
                    for sub_entry in sub_entries.filter_map(Result::ok) {
                        if let Some(ext) = sub_entry.path().extension() {
                            let ext_str = ext.to_string_lossy().to_lowercase();
                            if ext_str == "exe" {
                                exe_count += 1;
                            }
                            if ext_str == "dll" {
                                dll_count += 1;
                            }
                        }
                    }
                }

                // If this folder has multiple bypass files, it's likely the right one
                if exe_count > 0 || dll_count > 2 {
                    println!("📁 Found bypass files in subfolder: {}", path.display());
                    println!(
                        "   Contains: {} exe files, {} dll files",
                        exe_count, dll_count
                    );
                    return Ok(path.to_string_lossy().to_string());
                }
            }
        }
    }

    // Fallback to original extract path
    println!("⚠️  Using original extract path as fallback");
    Ok(extract_path.to_string())
}

// Copy bypass files directly to game directory (flatten folder structure)
fn copy_bypass_files_flat(src: &str, dst: &str) -> Result<(), String> {
    let src_path = Path::new(src);
    let dst_path = Path::new(dst);

    let mut files_replaced = 0;
    let mut files_new = 0;

    println!("📂 Installing bypass files from: {}", src_path.display());
    println!("📂 Target directory: {}", dst_path.display());

    for entry in WalkDir::new(src_path) {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_file() {
            // Get just the filename (no folder structure)
            let file_name = match path.file_name() {
                Some(name) => name,
                None => continue,
            };

            // Destination is directly in game root directory
            let dest_file = dst_path.join(file_name);

            println!("📄 Installing: {}", file_name.to_string_lossy());

            let file_exists = dest_file.exists();

            // Copy the file directly to game root (this will overwrite existing files)
            fs::copy(path, &dest_file).map_err(|e| e.to_string())?;

            if file_exists {
                println!("   🔄 REPLACED existing file");
                files_replaced += 1;
            } else {
                println!("   ✅ Added new file");
                files_new += 1;
            }
        }
    }

    println!("📊 Installation Summary:");
    println!("   🔄 Files replaced: {}", files_replaced);
    println!("   ✅ New files added: {}", files_new);

    Ok(())
}

async fn find_game_executable(game_path: &str) -> Option<String> {
    println!("Searching for game executable in: {}", game_path);

    let mut potential_executables = Vec::new();

    for entry in WalkDir::new(game_path).max_depth(3) {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "exe" {
                        let file_name = path.file_name()?.to_string_lossy().to_lowercase();
                        let file_size = path.metadata().ok()?.len();

                        println!(
                            "Found executable: {} (size: {} bytes)",
                            file_name, file_size
                        );

                        // Skip common non-game executables with more comprehensive list
                        let skip_patterns = [
                            "unins",
                            "setup",
                            "installer",
                            "redist",
                            "vcredist",
                            "directx",
                            "7za.exe",
                            "7z.exe",
                            "winrar",
                            "rar",
                            "zip",
                            "unzip",
                            "crash",
                            "report",
                            "dump",
                            "log",
                            "update",
                            "patch",
                            "config",
                            "settings",
                            "steam",
                            "origin",
                            "uplay",
                            "epic",
                            "gog",
                            "battle",
                            "blizzard",
                            "nvidia",
                            "amd",
                            "intel",
                            "microsoft",
                            "visual",
                            "dotnet",
                            "framework",
                            "runtime",
                            "service",
                            "helper",
                            "tool",
                            "util",
                            "support",
                            "driver",
                            "ubisoftconnect",
                            "uplayinstaller",
                        ];

                        let should_skip = skip_patterns
                            .iter()
                            .any(|pattern| file_name.contains(pattern));

                        // Debug: show what we're checking
                        println!(
                            "   🔍 Checking: {} (size: {:.1} MB)",
                            file_name,
                            file_size as f64 / 1_048_576.0
                        );
                        println!("      Should skip: {}", should_skip);
                        println!(
                            "      Size check: {} > 1MB = {}",
                            file_size,
                            file_size > 1_000_000
                        );

                        // Only consider larger executables (likely main game executables)
                        if !should_skip && file_size > 1_000_000 {
                            // At least 1MB, but we'll sort by size
                            potential_executables.push((
                                path.to_string_lossy().to_string(),
                                file_size,
                                file_name.clone(),
                            ));
                            println!(
                                "   ✅ ADDED as potential executable: {} ({:.1} MB)",
                                file_name,
                                file_size as f64 / 1_048_576.0
                            );
                        } else if should_skip {
                            println!("   ⏭️  SKIPPED (utility file): {}", file_name);
                        } else {
                            println!(
                                "   ⏭️  SKIPPED (too small): {} ({:.1} MB)",
                                file_name,
                                file_size as f64 / 1_048_576.0
                            );
                        }
                    }
                }
            }
        }
    }

    if potential_executables.is_empty() {
        println!("❌ No suitable game executable found");
        return None;
    }

    // Sort by file size (LARGEST FIRST - most likely to be the main game)
    potential_executables.sort_by(|a, b| b.1.cmp(&a.1));

    // Log all potential executables sorted by size
    println!("🎯 Potential game executables found (sorted by size):");
    for (i, (_path, size, name)) in potential_executables.iter().enumerate() {
        let size_mb = *size as f64 / 1_048_576.0;
        if i == 0 {
            println!(
                "  🏆 #{}: {} ({:.1} MB) - SELECTED (LARGEST)",
                i + 1,
                name,
                size_mb
            );
        } else {
            println!("  📄 #{}: {} ({:.1} MB)", i + 1, name, size_mb);
        }
    }

    // Return the largest executable (index 0 after sorting)
    let selected = &potential_executables[0];
    let selected_size_mb = selected.1 as f64 / 1_048_576.0;

    println!(
        "✅ FINAL SELECTION: {} ({:.1} MB)",
        selected.2, selected_size_mb
    );
    println!("   📁 Path: {}", selected.0);

    Some(selected.0.clone())
}

fn cleanup_temp_files(download_path: &str, extract_path: &str) -> Result<(), String> {
    // Remove download file
    if Path::new(download_path).exists() {
        fs::remove_file(download_path).map_err(|e| format!("Failed to cleanup download: {}", e))?;
    }

    // Remove extract directory
    if Path::new(extract_path).exists() {
        fs::remove_dir_all(extract_path)
            .map_err(|e| format!("Failed to cleanup extract: {}", e))?;
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct GameExecutable {
    name: String,
    path: String,
    size_mb: f64,
}

#[command]
async fn get_game_executables(game_path: String) -> Result<Vec<GameExecutable>, String> {
    println!("🔍 Scanning for executable files in: {}", game_path);

    if !Path::new(&game_path).exists() {
        return Err("Game folder does not exist".to_string());
    }

    let mut executables = Vec::new();

    for entry in WalkDir::new(&game_path).max_depth(2) {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "exe" {
                        if let Some(file_name) = path.file_name() {
                            let file_name_str = file_name.to_string_lossy().to_string();
                            let file_size = path.metadata().map(|m| m.len()).unwrap_or(0);
                            let size_mb = file_size as f64 / 1_048_576.0;

                            println!("📄 Found .exe: {} ({:.1} MB)", file_name_str, size_mb);

                            executables.push(GameExecutable {
                                name: file_name_str,
                                path: path.to_string_lossy().to_string(),
                                size_mb,
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort by size (largest first)
    executables.sort_by(|a, b| {
        b.size_mb
            .partial_cmp(&a.size_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    println!("🎯 Found {} executable files:", executables.len());
    for (i, exe) in executables.iter().enumerate() {
        if i == 0 {
            println!(
                "  🏆 {}: {} ({:.1} MB) - LARGEST",
                i + 1,
                exe.name,
                exe.size_mb
            );
        } else {
            println!("  📄 {}: {} ({:.1} MB)", i + 1, exe.name, exe.size_mb);
        }
    }

    Ok(executables)
}

#[command]
async fn check_bypass_installed_command(app_id: String) -> Result<bool, String> {
    check_bypass_installed(&app_id).await
}

#[command]
async fn confirm_and_launch_game(
    executable_path: String,
    game_name: String,
) -> Result<String, String> {
    println!("🎮 User confirmed to launch game: {}", game_name);
    println!("📁 Executable path: {}", executable_path);

    launch_game_executable(executable_path).await
}

#[command]
async fn launch_game_executable(executable_path: String) -> Result<String, String> {
    println!("🚀 Attempting to launch game: {}", executable_path);

    // Validate file exists
    if !Path::new(&executable_path).exists() {
        let error_msg = format!("Game executable not found: {}", executable_path);
        println!("❌ {}", error_msg);
        return Err(error_msg);
    }

    // Validate it's an .exe file
    if !executable_path.to_lowercase().ends_with(".exe") {
        let error_msg = format!("File is not an executable (.exe): {}", executable_path);
        println!("❌ {}", error_msg);
        return Err(error_msg);
    }

    // Check file size (should be reasonable for a game executable)
    if let Ok(metadata) = std::fs::metadata(&executable_path) {
        let file_size = metadata.len();
        println!(
            "📊 Executable size: {:.2} MB",
            file_size as f64 / 1_048_576.0
        );

        if file_size < 500_000 {
            // Less than 500KB seems too small for a game
            println!("⚠️  Warning: Executable seems very small for a game");
        }
    }

    #[cfg(target_os = "windows")]
    {
        println!("🎮 Launching game executable...");
        match Command::new(&executable_path)
            .current_dir(
                Path::new(&executable_path)
                    .parent()
                    .unwrap_or(Path::new(".")),
            )
            .spawn()
        {
            Ok(child) => {
                println!("✅ Game process started successfully!");
                println!("   PID: {:?}", child.id());
                println!("   Path: {}", executable_path);
                println!("   Working Dir: {:?}", Path::new(&executable_path).parent());

                // Don't wait for the game to finish, just confirm it started
                Ok(
                    "Game launched successfully! The game is now running with bypass enabled."
                        .to_string(),
                )
            }
            Err(e) => {
                let error_msg = format!("Failed to launch game: {}", e);
                println!("❌ {}", error_msg);
                Err(error_msg)
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Game launching is only supported on Windows".to_string())
    }
}

// ====================== END BYPASS FUNCTIONS ======================

// ====================== UPDATER FUNCTIONS ======================

#[command]
async fn check_for_updates(app: tauri::AppHandle) -> Result<String, String> {
    match app.updater() {
        Ok(updater) => {
            match updater.check().await {
                Ok(Some(update)) => {
                    Ok(format!("Update available: {} -> {}", 
                        update.current_version, 
                        update.version))
                }
                Ok(None) => {
                    Ok("No updates available".to_string())
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    if error_msg.contains("Could not fetch a valid release JSON") {
                        Err("Update service temporarily unavailable. Please try again later.".to_string())
                    } else if error_msg.contains("network") || error_msg.contains("connection") {
                        Err("Unable to connect to update server. Please check your internet connection.".to_string())
                    } else {
                        Err(format!("Update check unavailable: {}", e))
                    }
                }
            }
        }
        Err(e) => Err("Update service is not available at this time.".to_string())
    }
}

#[command]
async fn install_update(app: tauri::AppHandle) -> Result<String, String> {
    match app.updater() {
        Ok(updater) => {
            match updater.check().await {
                Ok(Some(update)) => {
                    match update.download_and_install(
                        |_chunk_length, _content_length| {
                            // Progress callback - bisa ditambahkan emit event ke frontend
                        },
                        || {
                            // Download finished callback
                        }
                    ).await {
                        Ok(_) => Ok("Update installed successfully. Please restart the application.".to_string()),
                        Err(e) => {
                            let error_msg = e.to_string();
                            if error_msg.contains("network") || error_msg.contains("download") {
                                Err("Download failed. Please check your internet connection and try again.".to_string())
                            } else {
                                Err("Installation failed. Please try again or contact support.".to_string())
                            }
                        }
                    }
                }
                Ok(None) => {
                    Err("No updates available to install".to_string())
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    if error_msg.contains("Could not fetch a valid release JSON") {
                        Err("Update service temporarily unavailable. Please try again later.".to_string())
                    } else if error_msg.contains("network") || error_msg.contains("connection") {
                        Err("Unable to connect to update server. Please check your internet connection.".to_string())
                    } else {
                        Err(format!("Update check unavailable: {}", e))
                    }
                }
            }
        }
        Err(e) => Err("Update service is not available at this time.".to_string())
    }
}

// ====================== END UPDATER FUNCTIONS ======================

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            greet,
            download_game,
            search_games,
            get_game_details,
            get_library_games,
            check_game_in_library,
            initialize_app,
            restart_steam,
            check_steam_status,
            get_game_dlc_list,
            get_batch_game_details,
            get_dlcs_in_lua,
            sync_dlcs_in_lua,
            clear_cache,
            refresh_cache_background,
            remove_game,
            check_bypass_availability,
            check_bypass_installed_command,
            install_bypass,
            launch_game_executable,
            confirm_and_launch_game,
            get_game_executables,
            check_for_updates,
            install_update,
            commands::update_game_files
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
