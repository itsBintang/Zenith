// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod database;
mod models;
mod bypass;
mod steam_utils;
mod download;
mod hydra_api;
mod catalogue_commands;
mod metadata_service;

use crate::steam_utils::{find_steam_config_path, find_steam_executable_path, update_lua_files};
use crate::download::{DownloadManagerState};
use futures::stream::{self, StreamExt};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Cursor;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tauri::command;
use tempfile::TempDir;
use walkdir::WalkDir;
use zip::ZipArchive;
use tauri_plugin_updater::UpdaterExt;

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

// Old GameCache struct and CacheEntry removed - now using SQLite with granular TTL
// All caching is now handled by database::legacy_adapter::LegacyGameCacheAdapter

lazy_static::lazy_static! {
    static ref GAME_CACHE: database::legacy_adapter::LegacyGameCacheAdapter = {
        database::legacy_adapter::LegacyGameCacheAdapter::new()
            .expect("Failed to initialize SQLite game cache")
    };
    pub static ref HTTP_CLIENT: reqwest::Client = {
        reqwest::Client::builder()
            .user_agent("zenith-launcher/1.0")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client")
    };
    pub static ref DOWNLOAD_CLIENT: reqwest::Client = {
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
    repos.push(("SteamAutoCracks/ManifestHub".to_string(), RepoType::Branch));
    repos.push((
        "https://raw.githubusercontent.com/sushi-dev55/sushitools-games-repo/refs/heads/main/"
            .to_string(),
        RepoType::DirectZip,
    ));
    repos.push(("Fairyvmos/bruh-hub".to_string(), RepoType::Branch));
    repos.push(("itsBintang/ManifestHub".to_string(), RepoType::Branch));
    repos.push((
        "https://mellyiscoolaf.pythonanywhere.com/".to_string(),
        RepoType::DirectUrl,
    ));
    repos.push((
        "http://masss.pythonanywhere.com/storage?auth=IEOIJE54esfsipoE56GE4&appid=".to_string(),
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



#[derive(Debug, Serialize, Deserialize)]
struct InitProgress {
    step: String,
    progress: f32,
    completed: bool,
}

#[command]
async fn initialize_app(_app: tauri::AppHandle) -> Result<Vec<InitProgress>, String> {
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
                progress: 20.0,
                completed: true,
            });
        }
        Err(_) => {
            return Err("Steam installation not found. Please install Steam first.".to_string());
        }
    }

    // Step 2: Database ready
    progress_steps.push(InitProgress {
        step: "Database initialization complete".to_string(),
        progress: 45.0,
        completed: true,
    });

    // Step 3: Initialize cache system and run migration
    progress_steps.push(InitProgress {
        step: "Initializing SQLite cache system...".to_string(),
        progress: 55.0,
        completed: false,
    });

    // Run auto-migration from JSON to SQLite if needed
    match database::migration_utils::auto_migrate_if_needed() {
        Ok(Some(result)) => {
            println!("✅ Migration completed: {} game names, {} game details", 
                     result.game_names_migrated, result.game_details_migrated);
            progress_steps.push(InitProgress {
                step: format!("Migrated {} games from JSON to SQLite", 
                            result.game_names_migrated + result.game_details_migrated),
                progress: 65.0,
                completed: true,
            });
        }
        Ok(None) => {
            println!("✅ SQLite cache ready (no migration needed)");
        }
        Err(e) => {
            println!("⚠️  Migration failed, using fallback: {}", e);
        }
    }

    // Cleanup any expired cache entries
    GAME_CACHE.cleanup_expired();
    GAME_CACHE.cache_stats();

    progress_steps.push(InitProgress {
        step: "SQLite cache system ready".to_string(),
        progress: 70.0,
        completed: true,
    });

    // Step 4: Pre-load library with full game names (warm-up cache)
    progress_steps.push(InitProgress {
        step: "Loading game library...".to_string(),
        progress: 80.0,
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
            progress: 85.0,
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
    let is_circuit_open = GAME_CACHE.is_circuit_breaker_open().await;

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
    let is_circuit_open = GAME_CACHE.is_circuit_breaker_open().await;

    if is_circuit_open {
        // Return placeholder when circuit is open
        return Some(format!("Game {}", app_id));
    }

    // Get or create a request lock for this app_id to prevent duplicate requests
    let request_lock = GAME_CACHE.get_or_create_request_lock(app_id).await;
    let _guard = request_lock.lock().await;

    // Check cache again after acquiring lock (another request might have completed)
    if let Some(cached_name) = GAME_CACHE.get_game_name(app_id).await {
        GAME_CACHE.remove_request_lock(app_id).await;
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
                        GAME_CACHE.record_error().await;
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
                                GAME_CACHE.reset_error_count().await;
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
                GAME_CACHE.record_error().await;
                None
            }
        }
    }
    .await;

    // Clean up the request lock
    GAME_CACHE.remove_request_lock(app_id).await;

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
            GAME_CACHE.record_error().await;
            return Err(format!("Request failed: {}", e));
        }
    };

    if !resp.status().is_success() {
        if resp.status().as_u16() == 429 {
            GAME_CACHE.record_error().await;
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
    GAME_CACHE.reset_error_count().await;

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

#[command]
fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to Zenith!", name)
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
    // DLC list logging reduced to prevent spam
    // println!("Fetching DLC list for game: {}", app_id);

    // Check if we have cached game details with DLC
    if let Some(cached_details) = GAME_CACHE.get_game_details(&app_id).await {
        if !cached_details.dlc.is_empty() {
            // Cached DLC logging reduced to prevent spam
            // println!(
            //     "Found {} cached DLCs for game {}",
            //     cached_details.dlc.len(),
            //     app_id
            // );
            return Ok(cached_details.dlc);
        }
    }

    // Fetch DLC data from Steam API
    let url = format!(
        "https://store.steampowered.com/api/appdetails?appids={}",
        app_id
    );
    // Only log actual API calls, not cache hits
    // println!("Fetching DLC data from Steam API for: {}", app_id);

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
    // Batch details logging reduced to prevent spam
    // println!("Fetching batch details for {} DLCs", app_ids.len());
    let mut details_list = Vec::new();
    let mut _cache_hits = 0;
    let mut _api_calls = 0;

    // Process in smaller batches to avoid overwhelming the API
    for chunk in app_ids.chunks(5) {
        let mut batch_futures = Vec::new();

        for app_id in chunk {
            // Check cache first to count hits
            if GAME_CACHE.get_game_details(app_id).await.is_some() {
                _cache_hits += 1;
            } else {
                _api_calls += 1;
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

    // Batch completion logging reduced to prevent spam
    // println!(
    //     "Batch DLC fetch completed: {} cache hits, {} API calls, {} total results",
    //     cache_hits,
    //     api_calls,
    //     details_list.len()
    // );
    GAME_CACHE.cache_stats();
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

    // DLC installation logging reduced to prevent spam
    // println!(
    //     "Found {} installed DLCs for game {}",
    //     installed_dlcs.len(),
    //     app_id
    // );
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
async fn refresh_dlc_cache(app_id: String) -> Result<String, String> {
    // Force clear the specific game's DLC cache
    GAME_CACHE.invalidate_game_details(&app_id);
    
    // Fetch fresh DLC data from Steam API
    let url = format!(
        "https://store.steampowered.com/api/appdetails?appids={}",
        app_id
    );
    
    let resp = HTTP_CLIENT
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch from Steam API: {}", e))?;
        
    if !resp.status().is_success() {
        return Err(format!("Steam API returned status {}", resp.status()));
    }

    let v: serde_json::Value = resp.json().await
        .map_err(|e| format!("Failed to parse Steam API response: {}", e))?;
        
    let data = v
        .get(&app_id)
        .and_then(|x| x.get("data"))
        .ok_or("No data found in Steam API response")?;

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
        // Update cached game details with fresh DLC info
        if let Some(mut cached_details) = GAME_CACHE.get_game_details(&app_id).await {
            cached_details.dlc = dlc.clone();
            GAME_CACHE.set_game_details(app_id.clone(), cached_details);
        }
        
        Ok(format!("Successfully refreshed {} DLCs for game {}", dlc.len(), app_id))
    } else {
        Ok(format!("No DLCs found for game {}", app_id))
    }
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

#[derive(Debug, Clone, serde::Serialize)]
struct CacheDebugInfo {
    last_error_timestamp: Option<i64>,
}

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
        Err(_e) => Err("Update service is not available at this time.".to_string())
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
        Err(_e) => Err("Update service is not available at this time.".to_string())
    }
}

// ====================== CHANGELOG FUNCTIONS ======================

#[derive(Debug, Serialize, Deserialize)]
struct ChangelogEntry {
    version: String,
    changes: Vec<ChangelogItem>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChangelogItem {
    #[serde(rename = "type")]
    change_type: String,
    description: String,
    pr_number: Option<String>,
    details: Option<String>,
}

#[command]
async fn get_changelog() -> Result<ChangelogEntry, String> {
    // Try to get changelog from GitHub releases first
    match fetch_changelog_from_github().await {
        Ok(changelog) => Ok(changelog),
        Err(_) => {
            // Fallback to local changelog
            get_local_changelog()
        }
    }
}

async fn fetch_changelog_from_github() -> Result<ChangelogEntry, String> {
    // Try multiple possible repository URLs
    let possible_repos = vec![
        "itsBintang/Zenith",           // Primary repository
        "nazrilnazril/zenith-launcher",
        "nazril/zenith-launcher", 
        "nazrilnazril/Zenith",
        "nazril/Zenith",
    ];
    
    for repo in possible_repos {
        let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
        
        match try_fetch_changelog(&url).await {
            Ok(changelog) => {
                println!("✅ Successfully fetched changelog from: {}", repo);
                return Ok(changelog);
            }
            Err(e) => {
                println!("❌ Failed to fetch from {}: {}", repo, e);
                continue;
            }
        }
    }
    
    Err("Could not fetch changelog from any GitHub repository. Repository might be private or not exist.".to_string())
}

async fn try_fetch_changelog(url: &str) -> Result<ChangelogEntry, String> {
    let resp = HTTP_CLIENT
        .get(url)
        .header("User-Agent", "zenith-launcher/1.0")
        .header("Accept", "application/vnd.github.v3+json")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;
    
    if !resp.status().is_success() {
        return Err(format!("HTTP {}: {}", 
            resp.status().as_u16(), 
            resp.status().canonical_reason().unwrap_or("Unknown")
        ));
    }
    
    let release: serde_json::Value = resp.json().await
        .map_err(|e| format!("JSON parse error: {}", e))?;
    
    let version = release
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    
    let body = release
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    
    let changes = parse_github_changelog(&body);
    
    Ok(ChangelogEntry {
        version,
        changes,
    })
}

fn parse_github_changelog(body: &str) -> Vec<ChangelogItem> {
    let mut changes = Vec::new();
    let mut current_category = "Changed".to_string();
    
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        
        // Detect category headers (### Added, ### Fixed, etc.)
        if line.starts_with("### ") {
            let category = line[4..].trim();
            current_category = match category.to_lowercase().as_str() {
                "added" => "Added".to_string(),
                "improved" | "changed" => "Improved".to_string(),
                "fixed" | "bug fixes" => "Fixed".to_string(),
                "removed" | "deprecated" => "Removed".to_string(),
                "security" => "Security".to_string(),
                _ => "Changed".to_string(),
            };
            continue;
        }
        
        // Parse list items
        if line.starts_with("- ") || line.starts_with("* ") {
            let content = &line[2..].trim(); // Remove "- " or "* "
            
            // Extract PR number using regex pattern
            let pr_number = if let Some(start) = content.find("(#") {
                if let Some(end) = content[start..].find(')') {
                    Some(content[start..start + end + 1].to_string())
                } else {
                    None
                }
            } else {
                None
            };
            
            // Remove PR number from description for cleaner text
            let description = if let Some(pr_start) = content.find(" (#") {
                content[..pr_start].trim().to_string()
            } else {
                content.to_string()
            };
            
            // Skip empty descriptions
            if !description.is_empty() {
                changes.push(ChangelogItem {
                    change_type: current_category.clone(),
                    description: capitalize_first_letter(&description),
                    pr_number,
                    details: None,
                });
            }
        }
        
        // Handle single line changes without bullets (fallback)
        else if !line.starts_with("#") && !line.starts_with("**") && line.len() > 3 {
            // Skip common GitHub auto-generated content
            if line.contains("Full Changelog") || line.contains("compare/") {
                continue;
            }
            
            changes.push(ChangelogItem {
                change_type: current_category.clone(),
                description: capitalize_first_letter(&line),
                pr_number: None,
                details: None,
            });
        }
    }
    
    // If no changes found, add a default entry
    if changes.is_empty() {
        changes.push(ChangelogItem {
            change_type: "Updated".to_string(),
            description: "See release notes for details".to_string(),
            pr_number: None,
            details: Some(body.trim().to_string()),
        });
    }
    
    changes
}

fn capitalize_first_letter(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn get_local_changelog() -> Result<ChangelogEntry, String> {
    // Fallback changelog data
    let current_version = env!("CARGO_PKG_VERSION");
    
    Ok(ChangelogEntry {
        version: format!("v{}", current_version),
        changes: vec![
            ChangelogItem {
                change_type: "Added".to_string(),
                description: "custom title bar with modern design".to_string(),
                pr_number: Some("#45".to_string()),
                details: Some("Implemented a sleek custom title bar with transparency support, window controls, and an integrated menu system for better user experience.".to_string()),
            },
            ChangelogItem {
                change_type: "Added".to_string(),
                description: "dropdown menu with essential options".to_string(),
                pr_number: Some("#46".to_string()),
                details: Some("Added a comprehensive dropdown menu in the title bar with options for user guide, issue reporting, feature requests, project support, changelog viewing, and update checking.".to_string()),
            },
            ChangelogItem {
                change_type: "Improved".to_string(),
                description: "update system with better error handling".to_string(),
                pr_number: Some("#47".to_string()),
                details: Some("Enhanced the auto-update functionality with proper error handling, user-friendly messages, and progress feedback for a smoother update experience.".to_string()),
            },
            ChangelogItem {
                change_type: "Fixed".to_string(),
                description: "null pointer error in update check function".to_string(),
                pr_number: Some("#48".to_string()),
                details: Some("Resolved TypeError that occurred when the update service returned null values, ensuring stable operation of the update checking mechanism.".to_string()),
            },
        ],
    })
}

// ====================== END CHANGELOG FUNCTIONS ======================

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(DownloadManagerState::new())
        .setup(|_app| {
            // Database initialization moved to initialize_app function
            // to provide proper loading screen feedback
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            download_game,
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
            refresh_dlc_cache,
            refresh_cache_background,
            remove_game,
            commands::update_game_files,
            // Bypass Commands
            bypass::install_bypass,
            bypass::install_bypass_with_type,
            bypass::check_bypass_installed_command,
            bypass::get_game_installation_info,
            bypass::get_game_executables,
            bypass::get_bypass_notes,
            bypass::confirm_and_launch_game,
            bypass::launch_game_executable,
            check_for_updates,
            install_update,
            get_changelog,
            commands::update_game_files_enhanced,
            // Debug commands
            database::commands::debug_cache_entry,
            database::commands::force_clear_cache,
            // SQLite Database Management Commands
            database::commands::migrate_json_to_sqlite,
            database::commands::get_migration_status,
            database::commands::get_database_stats,
            database::commands::cleanup_expired_cache,
            database::commands::vacuum_database,
            database::commands::test_sqlite_connection,
            database::commands::restore_json_backup,
            // Safe Batch Processing Commands
            database::commands::batch_refresh_games,
            database::commands::smart_refresh_library,
            database::commands::get_cache_config,
            // Bypass Games Cache Commands
            database::commands::get_bypass_games_cached,
            database::commands::refresh_bypass_games_cache,
            database::commands::clear_bypass_games_cache,
            database::commands::get_bypass_game_by_id,
            database::commands::get_bypass_games_cache_stats,
            // Profile Management Commands
            commands::get_user_profile,
            commands::update_profile_field,
            commands::upload_profile_image,
            commands::get_profile_image_base64,
            commands::reset_profile_to_default,
            // Steam Path Management Commands
            commands::get_steam_path,
            commands::set_steam_path,
            commands::detect_steam_path,
            // Download Manager Commands
            download::initialize_download_manager,
            download::shutdown_download_manager,
            download::start_download,
            download::pause_download,
            download::resume_download,
            download::cancel_download,
            download::get_download_progress,
            download::get_all_downloads,
            download::get_active_downloads,
            download::is_download_manager_ready,
            download::detect_url_type,
            // Download History Commands
            database::history_commands::get_download_history,
            database::history_commands::get_download_history_stats,
            database::history_commands::search_download_history,
            database::history_commands::get_download_history_entry,
            database::history_commands::delete_download_history_entry,
            database::history_commands::clear_download_history,
            database::history_commands::redownload_from_history,
            database::history_commands::debug_history_database,
            // Catalogue Commands (Hydra API)
            catalogue_commands::get_catalogue_list,
            catalogue_commands::get_paginated_catalogue,
            catalogue_commands::search_catalogue_games,
            catalogue_commands::get_sample_catalogue_games,
            catalogue_commands::test_hydra_connection,
            // Metadata Commands
            metadata_service::get_metadata_resources,
            metadata_service::get_filter_metadata,
            metadata_service::test_metadata_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


