use crate::models::{RepoType, UpdateStrategy, UpdateSource, ManifestInfo, UpdateResult};
use anyhow::Result;
use regex::Regex;
// Removed unused imports
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::command;
use uuid::Uuid;
use walkdir::WalkDir;
use zip::ZipArchive;
use base64::prelude::*;

#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

#[command]
pub async fn update_game_files(app_id: String, game_name: String) -> Result<String, String> {
    println!("Starting update for AppID: {} ({})", app_id, game_name);

    let steam_config_path = find_steam_config_path().map_err(|e| e.to_string())?;
    let lua_file_path =
        find_lua_file_for_appid(&steam_config_path, &app_id).map_err(|e| e.to_string())?;

    // --- 1. Download Branch Zip ---
    let client = reqwest::Client::builder()
        .user_agent("zenith-updater/1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let mut repos = HashMap::new();
    repos.insert("Fairyvmos/bruh-hub".to_string(), RepoType::Branch);
    repos.insert("SteamAutoCracks/ManifestHub".to_string(), RepoType::Branch);
    repos.insert(
        "ManifestHub/ManifestHub".to_string(),
        RepoType::Decrypted,
    );

    let mut zip_content: Option<bytes::Bytes> = None;

    for (repo_full_name, _) in &repos {
        let api_url = format!(
            "https://api.github.com/repos/{}/zipball/{}",
            repo_full_name, app_id
        );
        println!("Trying to download from: {}", api_url);

        match client
            .get(&api_url)
            .timeout(Duration::from_secs(600))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                zip_content = Some(response.bytes().await.map_err(|e| e.to_string())?);
                println!("Successfully downloaded zip from {}", repo_full_name);
                break;
            }
            Ok(response) => {
                println!(
                    "Failed to download from {}. Status: {}",
                    repo_full_name,
                    response.status()
                );
                continue;
            }
            Err(e) => {
                println!("Error downloading from {}: {}", repo_full_name, e);
                continue;
            }
        }
    }

    let Some(zip_bytes) = zip_content else {
        return Err("Failed to download game data from all repositories.".to_string());
    };

    // --- 2. Extract Manifests ---
    let temp_dir = std::env::temp_dir().join(format!("zenith_update_{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

    let mut manifest_map: HashMap<String, String> = HashMap::new();
    let mut archive = ZipArchive::new(std::io::Cursor::new(zip_bytes)).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| e.to_string())?;
        let file_path = file
            .enclosed_name()
            .ok_or("Invalid file path in zip".to_string())?;

        if let Some(ext) = file_path.extension() {
            if ext == "manifest" {
                if let Some(file_name_os) = file_path.file_name() {
                    if let Some(file_name) = file_name_os.to_str() {
                        let re = Regex::new(r"(\d+)_(\d+)\.manifest").unwrap();
                        if let Some(caps) = re.captures(file_name) {
                            let depot_id = caps.get(1).unwrap().as_str().to_string();
                            let manifest_id = caps.get(2).unwrap().as_str().to_string();
                            manifest_map.insert(depot_id, manifest_id);
                        }
                    }
                }
            }
        }
    }

    if manifest_map.is_empty() {
        fs::remove_dir_all(&temp_dir).ok();
        return Err("No manifest files found in the downloaded archive.".to_string());
    }
    println!("Found {} new manifest IDs.", manifest_map.len());

    // --- 3. Update Lua File ---
    let original_lua_content = fs::read_to_string(&lua_file_path).map_err(|e| e.to_string())?;

    let mut updated_count = 0;
    let mut appended_count = 0;

    let re_replace = Regex::new(r#"setManifestid\s*\(\s*(\d+)\s*,\s*"(\d+)"\s*,\s*0\s*\)"#).unwrap();
    let mut processed_depots: HashMap<String, bool> = HashMap::new();

    let mut updated_lua_content = re_replace
        .replace_all(&original_lua_content, |caps: &regex::Captures| {
            let depot_id = caps.get(1).unwrap().as_str();
            let old_manifest_id = caps.get(2).unwrap().as_str();
            processed_depots.insert(depot_id.to_string(), true);

            if let Some(new_manifest_id) = manifest_map.get(depot_id) {
                if new_manifest_id != old_manifest_id {
                    updated_count += 1;
                    format!(r#"setManifestid({}, "{}", 0)"#, depot_id, new_manifest_id)
                } else {
                    caps.get(0).unwrap().as_str().to_string() // No change
                }
            } else {
                caps.get(0).unwrap().as_str().to_string() // No new manifest for this depot
            }
        })
        .to_string();

    let mut lines_to_append = Vec::new();
    for (depot_id, manifest_id) in &manifest_map {
        if !processed_depots.contains_key(depot_id) {
            lines_to_append.push(format!(
                r#"setManifestid({}, "{}", 0)"#,
                depot_id, manifest_id
            ));
            appended_count += 1;
        }
    }

    if !lines_to_append.is_empty() {
        updated_lua_content.push_str("\n-- Appended by Zenith Updater --\n");
        updated_lua_content.push_str(&lines_to_append.join("\n"));
        updated_lua_content.push('\n');
    }

    // --- 4. Save and Cleanup ---
    if updated_count > 0 || appended_count > 0 {
        fs::write(&lua_file_path, updated_lua_content).map_err(|e| e.to_string())?;
    }
    fs::remove_dir_all(&temp_dir).ok();

    if updated_count == 0 && appended_count == 0 {
        return Ok(format!("{} is already up to date.", game_name));
    }

    let result_message = format!(
        "Update for {} complete. Updated: {}, Appended: {}.",
        game_name, updated_count, appended_count
    );
    println!("{}", result_message);
    Ok(result_message)
}

fn find_steam_config_path() -> Result<PathBuf, anyhow::Error> {
    #[cfg(target_os = "windows")]
    {
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

        if let Ok(hkcu) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
            if let Ok(steam_path_str) = hkcu.get_value::<String, _>("SteamPath") {
                let config_path = PathBuf::from(steam_path_str).join("config");
                if config_path.exists() {
                    return Ok(config_path);
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home_dir) = dirs::home_dir() {
            let linux_paths = [".steam/steam/config", ".local/share/Steam/config"];
            let macos_path = "Library/Application Support/Steam/config";

            if cfg!(target_os = "linux") {
                for path in linux_paths.iter() {
                    let p = home_dir.join(path);
                    if p.exists() {
                        return Ok(p);
                    }
                }
            } else if cfg!(target_os = "macos") {
                let p = home_dir.join(macos_path);
                if p.exists() {
                    return Ok(p);
                }
            }
        }
    }

    Err(anyhow::anyhow!("Steam config directory not found."))
}

fn find_lua_file_for_appid(
    steam_config_path: &Path,
    app_id_to_find: &str,
) -> Result<PathBuf, anyhow::Error> {
    let stplugin_dir = steam_config_path.join("stplug-in");
    if !stplugin_dir.exists() {
        return Err(anyhow::anyhow!(
            "'stplug-in' directory not found in Steam config."
        ));
    }

    for entry in WalkDir::new(&stplugin_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_file() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "lua" {
                    if let Some(stem) = path.file_stem() {
                        if stem.to_string_lossy() == app_id_to_find {
                            return Ok(path.to_path_buf());
                        }
                    }

                    if let Ok(content) = fs::read_to_string(path) {
                        let re =
                            Regex::new(&format!(r"addappid\s*\(\s*({})\s*\)", app_id_to_find))
                                .unwrap();
                        if re.is_match(&content) {
                            return Ok(path.to_path_buf());
                        }
                    }
                }
            }
        }
    }

    Err(anyhow::anyhow!(format!(
        "Could not find a .lua file for AppID: {}",
        app_id_to_find
    )))
}

// Profile Management Commands using SQLite
#[command]
pub async fn get_user_profile() -> Result<crate::database::models::UserProfile, String> {
    use crate::database::{DatabaseManager, operations::UserProfileOperations};
    
    let db_path = get_profile_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    let profile = db_manager.with_connection(|conn| {
        match UserProfileOperations::get(conn)? {
            Some(profile) => Ok(profile),
            None => {
                // Create default profile if none exists
                let default_profile = crate::database::models::UserProfile::new(
                    "User".to_string(), 
                    Some("Steam User".to_string())
                );
                UserProfileOperations::upsert(conn, &default_profile)?;
                Ok(default_profile)
            }
        }
    }).map_err(|e| e.to_string())?;
    
    Ok(profile)
}

#[command]
pub async fn save_user_profile(profile: crate::database::models::UserProfile) -> Result<(), String> {
    use crate::database::{DatabaseManager, operations::UserProfileOperations};
    
    let db_path = get_profile_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    db_manager.with_connection(|conn| {
        UserProfileOperations::upsert(conn, &profile)
    }).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[command]
pub async fn upload_profile_image(image_data: Vec<u8>, image_type: String) -> Result<String, String> {
    use crate::database::{DatabaseManager, operations::UserProfileOperations};
    
    println!("Starting profile image upload - type: {}, size: {} bytes", image_type, image_data.len());
    
    // Validate image size (max 10MB)
    if image_data.len() > 10 * 1024 * 1024 {
        return Err("Image file too large. Maximum size is 10MB.".to_string());
    }
    
    // Validate image type
    let extension = match image_type.as_str() {
        "banner" => "banner.jpg",
        "avatar" => "avatar.jpg",
        _ => return Err("Invalid image type. Use 'banner' or 'avatar'".to_string()),
    };
    
    println!("Getting profile directory...");
    let profile_dir = get_profile_dir().map_err(|e| {
        println!("Failed to get profile directory: {}", e);
        e.to_string()
    })?;
    let image_path = profile_dir.join(extension);
    
    println!("Creating profile directory: {:?}", profile_dir);
    // Create profile directory if it doesn't exist
    fs::create_dir_all(&profile_dir).map_err(|e| {
        println!("Failed to create profile directory: {}", e);
        format!("Failed to create profile directory: {}", e)
    })?;
    
    println!("Writing image file to: {:?}", image_path);
    // Save image file
    fs::write(&image_path, image_data).map_err(|e| {
        println!("Failed to write image file: {}", e);
        format!("Failed to save image file: {}", e)
    })?;
    
    println!("Image file saved successfully, updating database...");
    // Update database with new image path
    let db_path = get_profile_db_path().map_err(|e| {
        println!("Failed to get database path: {}", e);
        e.to_string()
    })?;
    
    let db_manager = DatabaseManager::new(db_path).map_err(|e| {
        println!("Failed to create database manager: {}", e);
        e.to_string()
    })?;
    
    let field_name = match image_type.as_str() {
        "banner" => "banner_path",
        "avatar" => "avatar_path",
        _ => return Err("Invalid image type".to_string()),
    };
    
    println!("Updating database field: {}", field_name);
    db_manager.with_connection(|conn| {
        UserProfileOperations::update_field(conn, field_name, Some(&image_path.to_string_lossy().to_string()))
    }).map_err(|e| {
        println!("Failed to update database: {}", e);
        format!("Database update failed: {}", e)
    })?;
    
    println!("Profile image upload completed successfully: {:?}", image_path);
    // Return the file path
    Ok(image_path.to_string_lossy().to_string())
}

#[command]
pub async fn get_profile_image_path(image_type: String) -> Result<Option<String>, String> {
    let extension = match image_type.as_str() {
        "banner" => "banner.jpg",
        "avatar" => "avatar.jpg",
        _ => return Err("Invalid image type. Use 'banner' or 'avatar'".to_string()),
    };
    
    let profile_dir = get_profile_dir().map_err(|e| e.to_string())?;
    let image_path = profile_dir.join(extension);
    
    if image_path.exists() {
        Ok(Some(image_path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

#[command]
pub async fn get_profile_image_base64(image_type: String) -> Result<Option<String>, String> {
    let extension = match image_type.as_str() {
        "banner" => "banner.jpg",
        "avatar" => "avatar.jpg",
        _ => return Err("Invalid image type. Use 'banner' or 'avatar'".to_string()),
    };
    
    let profile_dir = get_profile_dir().map_err(|e| e.to_string())?;
    let image_path = profile_dir.join(extension);
    
    if image_path.exists() {
        let image_data = fs::read(&image_path).map_err(|e| e.to_string())?;
        let base64_data = base64::prelude::BASE64_STANDARD.encode(&image_data);
        Ok(Some(format!("data:image/jpeg;base64,{}", base64_data)))
    } else {
        Ok(None)
    }
}

#[command]
pub async fn reset_profile_to_default() -> Result<(), String> {
    use crate::database::{DatabaseManager, operations::UserProfileOperations};
    
    let db_path = get_profile_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    db_manager.with_connection(|conn| {
        // Reset to default profile
        let default_profile = crate::database::models::UserProfile::new(
            "User".to_string(), 
            Some("Steam User".to_string())
        );
        UserProfileOperations::upsert(conn, &default_profile)
    }).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[command]
pub async fn update_profile_field(field: String, value: Option<String>) -> Result<(), String> {
    use crate::database::{DatabaseManager, operations::UserProfileOperations};
    
    let db_path = get_profile_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    db_manager.with_connection(|conn| {
        UserProfileOperations::update_field(conn, &field, value.as_deref())
    }).map_err(|e| e.to_string())?;
    
    Ok(())
}

// Helper functions
fn get_profile_dir() -> Result<PathBuf, anyhow::Error> {
    // Use platform-specific app data directory
    let app_dir = if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Default\AppData\Roaming"))
    } else if cfg!(target_os = "macos") {
        dirs::home_dir()
            .map(|home| home.join("Library").join("Application Support"))
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    } else {
        dirs::home_dir()
            .map(|home| home.join(".local").join("share"))
            .unwrap_or_else(|| PathBuf::from("/tmp"))
    };
    
    Ok(app_dir.join("zenith").join("profile"))
}

fn get_profile_db_path() -> Result<PathBuf, anyhow::Error> {
    // Use the same database path as the main cache database
    let cache_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zenith-launcher")
        .join("cache");
    
    fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir.join("games.db"))
}

// Steam Path Management Commands
#[command]
pub async fn get_steam_path() -> Result<Option<String>, String> {
    use crate::database::{DatabaseManager, operations::CacheMetadataOperations};
    
    let db_path = get_profile_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    let steam_path = db_manager.with_connection(|conn| {
        CacheMetadataOperations::get(conn, "steam_path")
    }).map_err(|e| e.to_string())?;
    
    Ok(steam_path)
}

#[command]
pub async fn set_steam_path(path: String) -> Result<(), String> {
    use crate::database::{DatabaseManager, operations::CacheMetadataOperations};
    
    // Validate that the path exists and contains steam.exe
    let steam_exe_path = PathBuf::from(&path).join("steam.exe");
    if !steam_exe_path.exists() {
        return Err(format!("Invalid Steam path: steam.exe not found in {}", path));
    }
    
    let db_path = get_profile_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    db_manager.with_connection(|conn| {
        CacheMetadataOperations::set(conn, "steam_path", &path)
    }).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[command]
pub async fn detect_steam_path() -> Result<Option<String>, String> {
    // Try to auto-detect Steam installation path
    #[cfg(target_os = "windows")]
    {
        // Try registry first
        if let Ok(hklm) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam") {
            if let Ok(install_path) = hklm.get_value::<String, _>("InstallPath") {
                let steam_exe = PathBuf::from(&install_path).join("steam.exe");
                if steam_exe.exists() {
                    return Ok(Some(install_path));
                }
            }
        }
        
        if let Ok(hklm) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey("SOFTWARE\\Valve\\Steam") {
            if let Ok(install_path) = hklm.get_value::<String, _>("InstallPath") {
                let steam_exe = PathBuf::from(&install_path).join("steam.exe");
                if steam_exe.exists() {
                    return Ok(Some(install_path));
                }
            }
        }
        
        // Try common installation paths
        let common_paths = vec![
            "C:\\Program Files (x86)\\Steam",
            "C:\\Program Files\\Steam",
            "D:\\Steam",
            "E:\\Steam",
        ];
        
        for path in common_paths {
            let steam_exe = PathBuf::from(path).join("steam.exe");
            if steam_exe.exists() {
                return Ok(Some(path.to_string()));
            }
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home_dir) = dirs::home_dir() {
            let linux_paths = [".steam/steam", ".local/share/Steam"];
            let macos_path = "Library/Application Support/Steam";
            
            if cfg!(target_os = "linux") {
                for path in linux_paths.iter() {
                    let steam_path = home_dir.join(path);
                    if steam_path.exists() {
                        return Ok(Some(steam_path.to_string_lossy().to_string()));
                    }
                }
            } else if cfg!(target_os = "macos") {
                let steam_path = home_dir.join(macos_path);
                if steam_path.exists() {
                    return Ok(Some(steam_path.to_string_lossy().to_string()));
                }
            }
        }
    }
    
    Ok(None)
}

// ==================== ENHANCED UPDATE SYSTEM ====================

/// Get all available update sources with priority order
fn get_update_sources() -> Vec<UpdateSource> {
    vec![
        UpdateSource {
            name: "SteamAutoCracks/ManifestHub".to_string(),
            url: "https://api.github.com/repos/SteamAutoCracks/ManifestHub/zipball/".to_string(),
            repo_type: RepoType::Branch,
            priority: 1,
            is_reliable: true,
        },
        UpdateSource {
            name: "Fairyvmos/bruh-hub".to_string(),
            url: "https://api.github.com/repos/Fairyvmos/bruh-hub/zipball/".to_string(),
            repo_type: RepoType::Branch,
            priority: 2,
            is_reliable: true,
        },
        UpdateSource {
            name: "itsBintang/ManifestHub".to_string(),
            url: "https://api.github.com/repos/itsBintang/ManifestHub/zipball/".to_string(),
            repo_type: RepoType::Branch,
            priority: 3,
            is_reliable: true,
        },
        UpdateSource {
            name: "ManifestHub/ManifestHub".to_string(),
            url: "https://api.github.com/repos/ManifestHub/ManifestHub/zipball/".to_string(),
            repo_type: RepoType::Decrypted,
            priority: 4,
            is_reliable: true,
        },
        UpdateSource {
            name: "sushi-dev55/sushitools-games-repo".to_string(),
            url: "https://raw.githubusercontent.com/sushi-dev55/sushitools-games-repo/refs/heads/main/".to_string(),
            repo_type: RepoType::DirectZip,
            priority: 5,
            is_reliable: false,
        },
        UpdateSource {
            name: "mellyiscoolaf.pythonanywhere.com".to_string(),
            url: "https://mellyiscoolaf.pythonanywhere.com/".to_string(),
            repo_type: RepoType::DirectUrl,
            priority: 6,
            is_reliable: false,
        },
    ]
}

/// Download manifest from a single source
async fn download_from_source(
    client: &reqwest::Client,
    source: &UpdateSource,
    app_id: &str,
) -> Result<HashMap<String, ManifestInfo>, String> {
    let download_url = match source.repo_type {
        RepoType::Branch | RepoType::Decrypted => {
            format!("{}{}", source.url, app_id)
        }
        RepoType::DirectZip => {
            format!("{}{}.zip", source.url, app_id)
        }
        RepoType::DirectUrl => {
            format!("{}{}", source.url, app_id)
        }
        _ => return Err("Unsupported repo type".to_string()),
    };

    println!("📡 Downloading from: {} ({})", source.name, download_url);

    let response = client
        .get(&download_url)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    let zip_bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    extract_manifests_from_zip(&zip_bytes, &source.name)
}

/// Extract manifest information from ZIP
fn extract_manifests_from_zip(
    zip_bytes: &bytes::Bytes,
    source_name: &str,
) -> Result<HashMap<String, ManifestInfo>, String> {
    let mut manifest_map = HashMap::new();
    let mut archive = ZipArchive::new(std::io::Cursor::new(zip_bytes))
        .map_err(|e| format!("Failed to read ZIP: {}", e))?;

    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry: {}", e))?;
        
        let file_path = file
            .enclosed_name()
            .ok_or("Invalid file path in ZIP")?;

        if let Some(ext) = file_path.extension() {
            if ext == "manifest" {
                if let Some(file_name_os) = file_path.file_name() {
                    if let Some(file_name) = file_name_os.to_str() {
                        let re = Regex::new(r"(\d+)_(\d+)\.manifest").unwrap();
                        if let Some(caps) = re.captures(file_name) {
                            let depot_id = caps.get(1).unwrap().as_str().to_string();
                            let manifest_id = caps.get(2).unwrap().as_str().to_string();
                            
                            let manifest_info = ManifestInfo {
                                depot_id: depot_id.clone(),
                                manifest_id: manifest_id.clone(),
                                source: source_name.to_string(),
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs(),
                            };
                            
                            manifest_map.insert(depot_id, manifest_info);
                        }
                    }
                }
            }
        }
    }

    if manifest_map.is_empty() {
        return Err("No manifest files found".to_string());
    }

    Ok(manifest_map)
}

/// Compare manifests and find the newest for each depot
fn find_newest_manifests(
    all_manifests: Vec<HashMap<String, ManifestInfo>>,
) -> HashMap<String, ManifestInfo> {
    let mut newest_manifests: HashMap<String, ManifestInfo> = HashMap::new();

    // Flatten all manifests into a single collection
    for manifest_map in all_manifests {
        for (depot_id, manifest_info) in manifest_map {
            match newest_manifests.get(&depot_id) {
                Some(existing) => {
                    // Compare manifest IDs (newer usually means bigger number, but not always)
                    // For now, we'll use timestamp and fallback to string comparison
                    if manifest_info.timestamp > existing.timestamp ||
                       (manifest_info.timestamp == existing.timestamp && 
                        manifest_info.manifest_id > existing.manifest_id) {
                        newest_manifests.insert(depot_id, manifest_info);
                    }
                }
                None => {
                    newest_manifests.insert(depot_id, manifest_info);
                }
            }
        }
    }

    newest_manifests
}

/// Enhanced update function - now only uses smart update
#[command]
pub async fn update_game_files_enhanced(
    app_id: String,
    game_name: String,
    strategy: Option<String>,
) -> Result<UpdateResult, String> {
    println!("🚀 Starting smart update for {}", game_name);

    let steam_config_path = find_steam_config_path().map_err(|e| e.to_string())?;
    let lua_file_path = find_lua_file_for_appid(&steam_config_path, &app_id).map_err(|e| e.to_string())?;

    let client = reqwest::Client::builder()
        .user_agent("zenith-updater/2.0")
        .build()
        .map_err(|e| e.to_string())?;

    // Always use smart update
    smart_update(&client, &app_id, &game_name, &lua_file_path).await
}

/// Smart update - check all sources and use newest manifests automatically
async fn smart_update(
    client: &reqwest::Client,
    app_id: &str,
    game_name: &str,
    lua_file_path: &PathBuf,
) -> Result<UpdateResult, String> {
    println!("🧠 Smart update: Automatically getting latest version from all sources...");
    
    let sources = get_update_sources();
    let mut all_manifests = Vec::new();

    println!("🔍 Checking all sources for newest manifests...");

    for source in &sources {
        match download_from_source(client, source, app_id).await {
            Ok(manifests) => {
                println!("✅ Got {} manifests from {}", manifests.len(), source.name);
                all_manifests.push(manifests);
            }
            Err(e) => {
                println!("❌ Failed {}: {}", source.name, e);
            }
        }
    }

    if all_manifests.is_empty() {
        return Err("No sources provided manifests".to_string());
    }

    let newest_manifests = find_newest_manifests(all_manifests);
    let manifest_map: HashMap<String, String> = newest_manifests
        .iter()
        .map(|(k, v)| (k.clone(), v.manifest_id.clone()))
        .collect();

    let (updated_count, appended_count) = apply_manifests_to_lua(lua_file_path, &manifest_map)?;

    // Build source info
    let sources_used: Vec<String> = newest_manifests
        .values()
        .map(|m| m.source.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    Ok(UpdateResult {
        success: true,
        message: format!("✅ {} updated successfully!", game_name),
        updated_count,
        appended_count,
        source_used: sources_used.join(", "),
        strategy_used: UpdateStrategy::Smart,
        has_newer_available: false,
        newer_sources: vec![],
    })
}

/// Extract current manifests from lua file
fn extract_manifests_from_lua(lua_content: &str) -> HashMap<String, String> {
    let mut manifests = HashMap::new();
    let re = Regex::new(r#"setManifestid\s*\(\s*(\d+)\s*,\s*"([^"]+)"\s*,\s*0\s*\)"#).unwrap();
    
    for caps in re.captures_iter(lua_content) {
        let depot_id = caps.get(1).unwrap().as_str().to_string();
        let manifest_id = caps.get(2).unwrap().as_str().to_string();
        manifests.insert(depot_id, manifest_id);
    }
    
    manifests
}

/// Apply manifests to lua file
fn apply_manifests_to_lua(
    lua_file_path: &PathBuf,
    manifest_map: &HashMap<String, String>,
) -> Result<(u32, u32), String> {
    let original_lua_content = fs::read_to_string(lua_file_path).map_err(|e| e.to_string())?;
    let mut updated_count = 0;
    let mut appended_count = 0;

    let re_replace = Regex::new(r#"setManifestid\s*\(\s*(\d+)\s*,\s*"(\d+)"\s*,\s*0\s*\)"#).unwrap();
    let mut processed_depots: HashMap<String, bool> = HashMap::new();

    let mut updated_lua_content = re_replace
        .replace_all(&original_lua_content, |caps: &regex::Captures| {
            let depot_id = caps.get(1).unwrap().as_str();
            let old_manifest_id = caps.get(2).unwrap().as_str();
            processed_depots.insert(depot_id.to_string(), true);

            if let Some(new_manifest_id) = manifest_map.get(depot_id) {
                if new_manifest_id != old_manifest_id {
                    updated_count += 1;
                    format!(r#"setManifestid({}, "{}", 0)"#, depot_id, new_manifest_id)
                } else {
                    caps.get(0).unwrap().as_str().to_string()
                }
            } else {
                caps.get(0).unwrap().as_str().to_string()
            }
        })
        .to_string();

    let mut lines_to_append = Vec::new();
    for (depot_id, manifest_id) in manifest_map {
        if !processed_depots.contains_key(depot_id) {
            lines_to_append.push(format!(
                r#"setManifestid({}, "{}", 0)"#,
                depot_id, manifest_id
            ));
            appended_count += 1;
        }
    }

    if !lines_to_append.is_empty() {
        updated_lua_content.push_str("\n-- Appended by Zenith Enhanced Updater --\n");
        updated_lua_content.push_str(&lines_to_append.join("\n"));
        updated_lua_content.push('\n');
    }

    if updated_count > 0 || appended_count > 0 {
        fs::write(lua_file_path, updated_lua_content).map_err(|e| e.to_string())?;
    }

    Ok((updated_count, appended_count))
}

