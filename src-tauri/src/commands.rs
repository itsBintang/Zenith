use crate::models::RepoType;
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::command;
use uuid::Uuid;
use walkdir::WalkDir;
use zip::ZipArchive;

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

