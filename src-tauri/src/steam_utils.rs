use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use regex::Regex;
use walkdir::WalkDir;

#[cfg(target_os = "windows")]
use winreg::{enums::*, RegKey};

pub fn find_steam_installation_path() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(steam_key) = hklm.open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam") {
            if let Ok(install_path) = steam_key.get_value::<String, _>("InstallPath") {
                return Ok(install_path);
            }
        }
        if let Ok(steam_key) = hklm.open_subkey("SOFTWARE\\Valve\\Steam") {
            if let Ok(install_path) = steam_key.get_value::<String, _>("InstallPath") {
                return Ok(install_path);
            }
        }
    }
    // Fallback for non-Windows or if registry fails
    let common_paths = vec!["C:\\Program Files (x86)\\Steam", "C:\\Program Files\\Steam"];
    for path in common_paths {
        if Path::new(path).exists() {
            return Ok(path.to_string());
        }
    }
    Err("Steam installation not found".to_string())
}

pub async fn find_game_folder_from_acf(app_id: &str, steam_path: &str) -> Option<String> {
    let steamapps_path = format!("{}/steamapps", steam_path);
    let library_folders_path = format!("{}/libraryfolders.vdf", steamapps_path);

    let mut library_paths = vec![steamapps_path.clone()];

    if let Ok(content) = fs::read_to_string(&library_folders_path) {
        let re = Regex::new(r#""path"\s+"([^"]+)""#).unwrap();
        for cap in re.captures_iter(&content) {
            library_paths.push(format!("{}\\steamapps", &cap[1].replace("\\\\", "\\")));
        }
    }

    for path in library_paths {
        let acf_file = format!("{}/appmanifest_{}.acf", path, app_id);
        if let Ok(content) = fs::read_to_string(&acf_file) {
            let re = Regex::new(r#""installdir"\s+"([^"]+)""#).unwrap();
            if let Some(cap) = re.captures(&content) {
                return Some(cap[1].to_string());
            }
        }
    }
    None
}

pub fn find_steam_executable_path() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(steam_key) = hklm.open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam") {
            if let Ok(install_path) = steam_key.get_value::<String, _>("InstallPath") {
                let steam_exe = format!("{}\\steam.exe", install_path);
                if std::path::Path::new(&steam_exe).exists() {
                    return Ok(steam_exe);
                }
            }
        }

        if let Ok(steam_key) = hklm.open_subkey("SOFTWARE\\Valve\\Steam") {
            if let Ok(install_path) = steam_key.get_value::<String, _>("InstallPath") {
                let steam_exe = format!("{}\\steam.exe", install_path);
                if std::path::Path::new(&steam_exe).exists() {
                    return Ok(steam_exe);
                }
            }
        }

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
        Ok("steam".to_string())
    }
}

pub fn find_steam_config_path() -> Result<PathBuf, anyhow::Error> {
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

    #[cfg(target_os = "windows")]
    {
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

pub fn update_lua_files(
    stplugin_dir: &Path,
    app_id: &str,
    manifest_map: &HashMap<String, String>,
) -> Result<(), anyhow::Error> {
    if let Some(lua_file) = find_lua_file_for_appid(stplugin_dir, app_id)? {
        println!("Updating LUA file: {:?}", lua_file);

        let original_content = fs::read_to_string(&lua_file)?;
        let mut updated_content = original_content.clone();
        let mut updated_count = 0;

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

pub fn find_lua_file_for_appid(
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
                    if let Some(stem) = path.file_stem() {
                        if stem.to_string_lossy() == app_id {
                            return Ok(Some(path.to_path_buf()));
                        }
                    }

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
