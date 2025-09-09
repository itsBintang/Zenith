// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::command;
use walkdir::WalkDir;
use regex::Regex;
use tempfile::TempDir;
use zip::ZipArchive;
use tokio::time::sleep;
use futures::stream::{self, StreamExt};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
struct DownloadResult {
    success: bool,
    message: String,
    file_path: Option<String>,
}

#[derive(Debug, Clone)]
struct CacheEntry<T> {
    data: T,
    timestamp: u64,
    expires_at: u64,
}

impl<T> CacheEntry<T> {
    fn new(data: T, ttl_seconds: u64) -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        Self {
            data,
            timestamp: now,
            expires_at: now + ttl_seconds,
        }
    }
    
    fn is_expired(&self) -> bool {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        now > self.expires_at
    }
}

struct GameCache {
    game_details: Arc<Mutex<HashMap<String, CacheEntry<GameDetail>>>>,
    game_names: Arc<Mutex<HashMap<String, CacheEntry<String>>>>,
    in_flight_requests: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    last_request_time: Arc<Mutex<u64>>,
    request_delay_ms: u64,
}

impl GameCache {
    fn new() -> Self {
        Self {
            game_details: Arc::new(Mutex::new(HashMap::new())),
            game_names: Arc::new(Mutex::new(HashMap::new())),
            in_flight_requests: Arc::new(Mutex::new(HashMap::new())),
            last_request_time: Arc::new(Mutex::new(0)),
            request_delay_ms: 100, // 100ms between requests
        }
    }
    
    async fn get_game_details(&self, app_id: &str) -> Option<GameDetail> {
        let cache = self.game_details.lock().unwrap();
        if let Some(entry) = cache.get(app_id) {
            if !entry.is_expired() {
                return Some(entry.data.clone());
            }
        }
        None
    }
    
    fn set_game_details(&self, app_id: String, details: GameDetail) {
        let mut cache = self.game_details.lock().unwrap();
        cache.insert(app_id.clone(), CacheEntry::new(details, 86400)); // 1 day TTL
        println!("Cached game details for: {}", app_id);
    }
    
    async fn get_game_name(&self, app_id: &str) -> Option<String> {
        let cache = self.game_names.lock().unwrap();
        if let Some(entry) = cache.get(app_id) {
            if !entry.is_expired() {
                return Some(entry.data.clone());
            }
        }
        None
    }
    
    fn set_game_name(&self, app_id: String, name: String) {
        let mut cache = self.game_names.lock().unwrap();
        cache.insert(app_id.clone(), CacheEntry::new(name, 604800)); // 7 days TTL
        println!("Cached game name for: {}", app_id);
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
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        
        let delay = {
            let last_request = self.last_request_time.lock().unwrap();
            if now - *last_request < self.request_delay_ms {
                Some(self.request_delay_ms - (now - *last_request))
            } else {
                None
            }
        }; // Lock is dropped here
        
        if let Some(delay_ms) = delay {
            println!("Throttling request, sleeping for {}ms", delay_ms);
            sleep(Duration::from_millis(delay_ms)).await;
        }
        
        // Update the last request time
        {
            let mut last_request = self.last_request_time.lock().unwrap();
            *last_request = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        } // Lock is dropped here
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
        
        println!("Cache cleanup: Details {}->{}, Names {}->{}", 
                details_before, details_after, names_before, names_after);
    }
    
    fn cache_stats(&self) {
        let details_cache = self.game_details.lock().unwrap();
        let names_cache = self.game_names.lock().unwrap();
        let in_flight = self.in_flight_requests.lock().unwrap();
        
        println!("Cache stats: {} game details, {} game names, {} in-flight requests", 
                details_cache.len(), names_cache.len(), in_flight.len());
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
}

#[command]
async fn download_game(app_id: String, game_name: String) -> Result<DownloadResult, String> {
    println!("Starting seamless installation for AppID: {} ({})", app_id, game_name);
    
    // Setup repositories to try
    let mut repos = HashMap::new();
    repos.insert("Fairyvmos/bruh-hub".to_string(), RepoType::Branch);
    repos.insert("SteamAutoCracks/ManifestHub".to_string(), RepoType::Branch);
    
    // Use global HTTP client
    
    // Try downloading from repositories
    for (repo_name, repo_type) in &repos {
        println!("Trying repository: {}", repo_name);
        
        match repo_type {
            RepoType::Branch => {
                let api_url = format!("https://api.github.com/repos/{}/zipball/{}", repo_name, app_id);
                println!("Downloading from: {}", api_url);
                
                match HTTP_CLIENT.get(&api_url)
                    .timeout(std::time::Duration::from_secs(60))
                    .send()
                    .await {
                    Ok(response) => {
                        if response.status().is_success() {
                            let bytes = response.bytes().await.map_err(|e| e.to_string())?;
                            
                            // Process ZIP in memory and install to Steam
                            match process_and_install_to_steam(&bytes, &app_id, &game_name).await {
                                Ok(install_info) => {
                                    return Ok(DownloadResult {
                                        success: true,
                                        message: format!("Successfully installed {} to Steam! {}", game_name, install_info),
                                        file_path: None, // No local file saved
                                    });
                                }
                                Err(e) => {
                                    println!("Failed to install to Steam: {}", e);
                                    continue; // Try next repository
                                }
                            }
                        } else {
                            println!("Failed to download from {}: HTTP {}", repo_name, response.status());
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
        message: format!("No data found for {} (AppID: {}) in any repository", game_name, app_id),
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
        // If numeric: treat as AppID direct
        if term.chars().all(|c| c.is_ascii_digit()) {
            println!("Searching by AppID: {}", term);
            let name = fetch_game_name_simple(&term).await.unwrap_or_else(|| format!("Unknown Game ({})", term));
            let header = header_image_for(&term);
            all_results.push(SearchResultItem { 
                app_id: term.clone(), 
                name, 
                header_image: header 
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
                            item.get("name").and_then(|v| v.as_str())
                        ) {
                            let app_id = id.to_string();
                            
                            // Filter out non-games unless explicitly searched for
                            let name_lower = name.to_lowercase();
                            let query_lower = query.to_lowercase();
                            
                            let is_non_game = [
                                "dlc", "soundtrack", "demo", "pack", "sdk", "artbook", 
                                "trailer", "movie", "beta", "ost", "wallpaper", "season pass",
                                "bonus content", "pre-purchase", "pre-order", "expansion"
                            ].iter().any(|&keyword| name_lower.contains(keyword));

                            let searching_for_non_game = [
                                "dlc", "soundtrack", "demo", "pack", "artbook", "trailer", 
                                "movie", "beta", "pass", "expansion"
                            ].iter().any(|&keyword| query_lower.contains(keyword));

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
                            "dlc", "soundtrack", "demo", "pack", "sdk", "artbook", 
                            "trailer", "movie", "beta", "ost", "wallpaper", "season pass",
                            "bonus content", "pre-purchase", "pre-order"
                        ].iter().any(|&keyword| name_lower.contains(keyword));

                        let searching_for_non_game = [
                            "dlc", "soundtrack", "demo", "pack", "artbook", "trailer", 
                            "movie", "beta", "pass"
                        ].iter().any(|&keyword| query_lower.contains(keyword));

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
            step: format!("Pre-loading {} games...", app_ids.len()),
            progress: 80.0,
            completed: false,
        });
        
        // Pre-fetch all game names with controlled concurrency
        if !app_ids.is_empty() {
            let games: Vec<_> = stream::iter(app_ids.clone())
                .map(|app_id| async move {
                    fetch_game_name_simple(&app_id).await
                })
                .buffer_unordered(6) // Slightly lower concurrency for init
                .collect()
                .await;
            
            let loaded_count = games.iter().filter(|g| g.is_some()).count();
            println!("Pre-loaded {} out of {} game names during initialization", loaded_count, app_ids.len());
        }
        
        app_ids.len()
    } else {
        0
    };
    
    progress_steps.push(InitProgress {
        step: format!("Ready! {} games loaded", game_count),
        progress: 100.0,
        completed: true,
    });
    
    println!("App initialization completed. Pre-loaded {} games.", game_count);
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

    println!("Found {} games in library", app_ids.len());

    // Process games with controlled concurrency (max 8 concurrent requests)
    let mut games: Vec<LibraryGame> = stream::iter(app_ids)
        .map(|app_id| async move {
            // Fetch game name, fall back to AppID if not found
            let name = fetch_game_name_simple(&app_id)
                .await
                .unwrap_or_else(|| format!("Unknown Game ({})", app_id));

            LibraryGame {
                app_id: app_id.clone(),
                name,
                header_image: header_image_for(&app_id),
            }
        })
        .buffer_unordered(8) // Max 8 concurrent requests
        .collect()
        .await;

    // Sort games by name alphabetically
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    // Final cache stats
    GAME_CACHE.cache_stats();
    println!("Successfully processed {} games", games.len());
    Ok(games)
}

fn header_image_for(app_id: &str) -> String {
    format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg", app_id)
}

async fn fetch_game_name_simple(app_id: &str) -> Option<String> {
    // Check cache first
    if let Some(cached_name) = GAME_CACHE.get_game_name(app_id).await {
        return Some(cached_name);
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
    
    let url = format!("https://store.steampowered.com/api/appdetails?appids={}", app_id);
    println!("Fetching game name from Steam API: {}", app_id);
    
    let result = async {
        let resp = HTTP_CLIENT.get(url).send().await.ok()?;
        if !resp.status().is_success() { return None; }
        let v: serde_json::Value = resp.json().await.ok()?;
        let data = v.get(app_id)?.get("data")?;
        let name = data.get("name")?.as_str().map(|s| s.to_string())?;
        
        // Cache the result
        GAME_CACHE.set_game_name(app_id.to_string(), name.clone());
        
        Some(name)
    }.await;
    
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
    
    let url = format!("https://store.steampowered.com/api/appdetails?appids={}", app_id);
    println!("Fetching game details from Steam API: {}", app_id);
    
    let resp = HTTP_CLIENT.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() { return Err(format!("status {}", resp.status())); }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let data = v.get(&app_id).and_then(|x| x.get("data")).ok_or("no data")?;

    let name = data.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let header_image = data.get("header_image").and_then(|x| x.as_str()).unwrap_or(&header_image_for(&app_id)).to_string();
    
    // Use background image for banner (higher resolution)
    let banner_image = data.get("background").and_then(|x| x.as_str())
        .or_else(|| data.get("background_raw").and_then(|x| x.as_str()))
        .unwrap_or(&header_image).to_string();
    
    let detailed_description = data.get("detailed_description").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let release_date = data.get("release_date").and_then(|x| x.get("date")).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let publisher = data.get("publishers").and_then(|x| x.get(0)).and_then(|x| x.as_str()).unwrap_or("").to_string();

    let trailer = data.get("movies").and_then(|arr| arr.get(0)).and_then(|item| item.get("mp4")).and_then(|mp4| mp4.get("max")).and_then(|x| x.as_str()).map(|s| s.to_string());
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
        let minimum = pc_req.get("minimum").and_then(|x| x.as_str()).map(|s| s.to_string());
        let recommended = pc_req.get("recommended").and_then(|x| x.as_str()).map(|s| s.to_string());
        
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
    };
    
    // Cache the result
    GAME_CACHE.set_game_details(app_id, game_detail.clone());
    
    Ok(game_detail)
}

fn parse_sysreq_html(html: &str) -> Vec<(String, String)> {
    let lower = html.replace("<br>", "\n").replace("<strong>", "").replace("</strong>", "");
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

async fn process_and_install_to_steam(zip_bytes: &[u8], app_id: &str, _game_name: &str) -> Result<String, anyhow::Error> {
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
    for entry in WalkDir::new(temp_dir.path()).into_iter().filter_map(Result::ok) {
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
    
    Ok(format!("Files installed: {} LUA, {} manifests, {} BIN files", lua_count, manifest_count, bin_count))
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
        use winreg::{RegKey, enums::*};
        
        if let Ok(hkcu) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
            if let Ok(steam_path_str) = hkcu.get_value::<String, _>("SteamPath") {
                let config_path = PathBuf::from(steam_path_str).join("config");
                if config_path.exists() {
                    return Ok(config_path);
                }
            }
        }
    }
    
    Err(anyhow::anyhow!("Steam config directory not found. Please make sure Steam is installed."))
}

fn update_lua_files(stplugin_dir: &Path, app_id: &str, manifest_map: &HashMap<String, String>) -> Result<(), anyhow::Error> {
    // Find LUA file for this app ID
    if let Some(lua_file) = find_lua_file_for_appid(stplugin_dir, app_id)? {
        println!("Updating LUA file: {:?}", lua_file);
        
        let original_content = fs::read_to_string(&lua_file)?;
        let mut updated_content = original_content.clone();
        let mut updated_count = 0;
        
        // Update existing manifest IDs
        let re_replace = Regex::new(r#"setManifestid\s*\(\s*(\d+)\s*,\s*"(\d+)"\s*,\s*0\s*\)"#)?;
        updated_content = re_replace.replace_all(&updated_content, |caps: &regex::Captures| {
            let depot_id = caps.get(1).unwrap().as_str();
            let old_manifest_id = caps.get(2).unwrap().as_str();
            
            if let Some(new_manifest_id) = manifest_map.get(depot_id) {
                if new_manifest_id != old_manifest_id {
                    updated_count += 1;
                    return format!(r#"setManifestid({}, "{}", 0)"#, depot_id, new_manifest_id);
                }
            }
            caps.get(0).unwrap().as_str().to_string()
        }).to_string();
        
        // Append new manifest IDs that weren't in the file
        let existing_depots: Vec<String> = re_replace.captures_iter(&original_content)
            .map(|cap| cap[1].to_string())
            .collect();
        
        let mut new_lines = Vec::new();
        for (depot_id, manifest_id) in manifest_map {
            if !existing_depots.contains(depot_id) {
                new_lines.push(format!(r#"setManifestid({}, "{}", 0)"#, depot_id, manifest_id));
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

fn find_lua_file_for_appid(stplugin_dir: &Path, app_id: &str) -> Result<Option<PathBuf>, anyhow::Error> {
    for entry in WalkDir::new(stplugin_dir).max_depth(1).into_iter().filter_map(Result::ok) {
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
            Ok(steam_path) => {
                match Command::new(&steam_path).spawn() {
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
                println!("Steam status check: {}", if is_running { "Running" } else { "Not running" });
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
                println!("Steam status check: {}", if is_running { "Running" } else { "Not running" });
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
async fn clear_cache() -> Result<String, String> {
    GAME_CACHE.cleanup_expired();
    Ok("Cache cleared successfully".to_string())
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
                    println!("Warning: Failed to delete manifest file {}: {}", file_name_str, e);
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
                    println!("Warning: Failed to delete BIN file {}: {}", file_name_str, e);
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
            message: format!("Removed {} files: {}", removed_files.len(), removed_files.join(", ")),
            file_path: None,
        })
    }
}

fn main() {
    tauri::Builder::default()
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
            clear_cache,
            remove_game
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
