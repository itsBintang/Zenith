use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use crate::steam_utils::{find_game_folder_from_acf, find_steam_installation_path};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::command;
use tauri::Emitter;
use tauri_plugin_updater::UpdaterExt;
use walkdir::WalkDir;
use zip::ZipArchive;
use crate::DOWNLOAD_CLIENT;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BypassProgress {
    step: String,
    progress: f64,
    app_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BypassStatus {
    available: bool,
    installing: bool,
    installed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameExecutable {
    name: String,
    path: String,
    size_mb: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BypassNotes {
    has_notes: bool,
    instructions: String,
    recommended_exe: Option<String>,
    exe_list: Vec<GameExecutable>, // List of exe to show (from note or fallback)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BypassResult {
    pub success: bool,
    pub message: String,
    pub should_launch: bool,
    pub game_executable_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BypassType {
    Type1, // Files only - flat copy to root
    Type2, // Folders and files - preserve structure  
    Type3, // Advanced mixed content
    Type4, // Custom installation method
    Type5, // Registry/config based
    // Add more types as needed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BypassInfo {
    pub app_id: String,
    pub bypass_type: BypassType,
    pub file_suffix: u8, // 1, 2, 3, etc.
    pub description: String,
}

#[derive(Debug)]
enum BypassStructure {
    FlatFiles,
    FoldersWithContent,
    MixedContent,
}

lazy_static! {
    // Multiple bypass download sources with fallback mechanism
    pub static ref BYPASS_SOURCES: Vec<&'static str> = vec![
        "https://bypass.nzr.web.id",
        "https://bypass1.nzr.web.id",
    ];
}

pub async fn check_bypass_installed(app_id: &str) -> Result<bool, String> {
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
pub async fn get_available_bypass_types(app_id: String) -> Result<Vec<BypassInfo>, String> {
    println!("Getting available bypass types for AppID: {}", app_id);
    let available_bypasses = detect_available_bypass_types(&app_id).await;
    Ok(available_bypasses)
}

#[command]
pub async fn install_bypass(app_id: String, window: tauri::Window) -> Result<BypassResult, String> {
    install_bypass_with_type(app_id, None, window).await
}

#[command] 
pub async fn install_bypass_with_type(app_id: String, bypass_type: Option<u8>, window: tauri::Window) -> Result<BypassResult, String> {
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

    // Step 3: Check bypass availability and find working source
    emit_progress("Checking bypass availability...", 30.0);
    
    // Step 4: Download bypass from multiple sources  
    emit_progress("Downloading bypass files...", 40.0);

    let download_path = download_bypass_from_multiple_sources(&window, &app_id, bypass_type)
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

    // Step 6: Install bypass files with specific type
    emit_progress("Installing bypass to game directory...", 85.0);
    install_bypass_files_with_type(&extract_path, &game_path, bypass_type)
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

#[command]
pub async fn check_bypass_installed_command(app_id: String) -> Result<bool, String> {
    check_bypass_installed(&app_id).await
}

#[command]
pub async fn check_bypass_availability(app_id: String) -> Result<BypassStatus, String> {
    println!("Checking bypass availability for AppID: {}", app_id);

    // Check if bypass already installed first
    let is_installed = check_bypass_installed(&app_id).await.unwrap_or(false);

    // Check for available bypass types (numbered files)
    let available_bypasses = detect_available_bypass_types(&app_id).await;
    
    if !available_bypasses.is_empty() {
        println!("✅ Found {} bypass type(s) for AppID: {}", available_bypasses.len(), app_id);
        for bypass_info in &available_bypasses {
            println!("   Type {}: {}", bypass_info.file_suffix, bypass_info.description);
        }
        
        return Ok(BypassStatus {
            available: true,
            installing: false,
            installed: is_installed,
        });
    }

    println!("❌ No bypass types available from any source");
    Ok(BypassStatus {
        available: false,
        installing: false,
        installed: is_installed,
    })
}

async fn detect_available_bypass_types(app_id: &str) -> Vec<BypassInfo> {
    let mut available_bypasses = Vec::new();
    
    // Check for numbered bypass files (1-10 for now, can be extended)
    for bypass_number in 1..=10 {
        for source in BYPASS_SOURCES.iter() {
            let bypass_url = format!("{}/{}_{}.zip", source, app_id, bypass_number);
            
            match DOWNLOAD_CLIENT.head(&bypass_url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        println!("✅ Found bypass type {} at source: {}", bypass_number, source);
                        
                        // Create bypass info based on type number
                        let bypass_type = match bypass_number {
                            1 => BypassType::Type1,
                            2 => BypassType::Type2,
                            3 => BypassType::Type3,
                            4 => BypassType::Type4,
                            5 => BypassType::Type5,
                            _ => BypassType::Type1, // Default fallback
                        };
                        
                        let description = get_bypass_type_description(&bypass_type);
                        
                        let bypass_info = BypassInfo {
                            app_id: app_id.to_string(),
                            bypass_type,
                            file_suffix: bypass_number,
                            description,
                        };
                        
                        available_bypasses.push(bypass_info);
                        break; // Found this type, move to next number
                    }
                }
                Err(_) => {
                    // Continue to next source
                    continue;
                }
            }
        }
    }
    
    // Also check for legacy bypass (no number suffix)
    for source in BYPASS_SOURCES.iter() {
        let bypass_url = format!("{}/{}.zip", source, app_id);
        
        match DOWNLOAD_CLIENT.head(&bypass_url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    println!("✅ Found legacy bypass at source: {}", source);
                    
                    let bypass_info = BypassInfo {
                        app_id: app_id.to_string(),
                        bypass_type: BypassType::Type1, // Default to Type1 for legacy
                        file_suffix: 0, // 0 indicates legacy/no suffix
                        description: "Legacy bypass (auto-detect)".to_string(),
                    };
                    
                    available_bypasses.push(bypass_info);
                    break;
                }
            }
            Err(_) => continue,
        }
    }
    
    available_bypasses
}

fn get_bypass_type_description(bypass_type: &BypassType) -> String {
    match bypass_type {
        BypassType::Type1 => "Files only - flat copy to root".to_string(),
        BypassType::Type2 => "Folders and files - preserve structure".to_string(), 
        BypassType::Type3 => "Advanced mixed content".to_string(),
        BypassType::Type4 => "Custom installation method".to_string(),
        BypassType::Type5 => "Registry/config based".to_string(),
    }
}

#[command]
pub async fn get_game_executables(game_path: String) -> Result<Vec<GameExecutable>, String> {
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

    // Sort alphabetically (no priority)
    executables.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    println!("🎯 Found {} executable files:", executables.len());
    for (i, exe) in executables.iter().enumerate() {
        println!("  📄 {}: {} ({:.1} MB)", i + 1, exe.name, exe.size_mb);
    }

    Ok(executables)
}

#[command]
pub async fn get_bypass_notes(game_path: String) -> Result<BypassNotes, String> {
    println!("🔍 Looking for bypass notes in: {}", game_path);
    
    let note_txt_path = format!("{}/note.txt", game_path);
    let note_path = format!("{}/note", game_path);
    let mut exe_list = Vec::new();
    
    println!("📝 Checking for note.txt at: {}", note_txt_path);
    println!("📝 Checking for note at: {}", note_path);
    
    // Check for note.txt first, then note without extension
    let (found_note_path, has_note_file) = if Path::new(&note_txt_path).exists() {
        println!("✅ Found note.txt");
        (note_txt_path, true)
    } else if Path::new(&note_path).exists() {
        println!("✅ Found note (without extension)");
        (note_path, true)
    } else {
        println!("❌ No note file found");
        (String::new(), false)
    };
    
    if has_note_file {
        match fs::read_to_string(&found_note_path) {
            Ok(content) => {
                let trimmed_content = content.trim().to_string();
                println!("📝 Found bypass notes: {}", trimmed_content);
                
                // Try to extract recommended exe from content
                let recommended_exe = extract_recommended_exe(&trimmed_content);
                
                // If we found an exe in note, create exe_list with only that exe
                if let Some(ref exe_name) = recommended_exe {
                    let exe_path = format!("{}/{}", game_path, exe_name);
                    if Path::new(&exe_path).exists() {
                        if let Ok(metadata) = fs::metadata(&exe_path) {
                            let size_mb = metadata.len() as f64 / 1_048_576.0;
                            exe_list.push(GameExecutable {
                                name: exe_name.clone(),
                                path: exe_path,
                                size_mb,
                            });
                            println!("🎯 Using ONLY exe from note: {} ({:.1} MB)", exe_name, size_mb);
                        } else {
                            println!("❌ Exe from note exists but cannot read metadata: {}", exe_name);
                        }
                    } else {
                        println!("❌ Exe from note not found at path: {}", exe_path);
                        // Don't fallback to scanning - if note specifies an exe, only use that
                    }
                } else {
                    println!("⚠️ No exe extracted from note content, will not show any exe");
                    // If note exists but no exe found, don't show any executables
                }
                
                Ok(BypassNotes {
                    has_notes: true,
                    instructions: trimmed_content,
                    recommended_exe,
                    exe_list,
                })
            }
            Err(e) => {
                println!("❌ Failed to read note.txt: {}", e);
                // Fallback to scanning all exe
                exe_list = scan_executables(&game_path).await?;
                Ok(BypassNotes {
                    has_notes: false,
                    instructions: "".to_string(),
                    recommended_exe: None,
                    exe_list,
                })
            }
        }
    } else {
        println!("📝 No note.txt found, scanning for executables...");
        // Fallback to scanning all exe
        exe_list = scan_executables(&game_path).await?;
        Ok(BypassNotes {
            has_notes: false,
            instructions: "".to_string(), 
            recommended_exe: None,
            exe_list,
        })
    }
}

async fn scan_executables(game_path: &str) -> Result<Vec<GameExecutable>, String> {
    let mut executables = Vec::new();

    for entry in WalkDir::new(game_path).max_depth(2) {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "exe" {
                        if let Some(file_name) = path.file_name() {
                            let file_name_str = file_name.to_string_lossy().to_string();
                            let file_size = path.metadata().map(|m| m.len()).unwrap_or(0);
                            let size_mb = file_size as f64 / 1_048_576.0;

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

    // Sort alphabetically (no priority)
    executables.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    
    Ok(executables)
}

fn extract_recommended_exe(content: &str) -> Option<String> {
    println!("🔍 Extracting exe from note content: '{}'", content);
    
    // First, try to find any .exe file mentioned
    if let Some(exe_pos) = content.find(".exe") {
        // Find the start of the exe name by looking backwards for a space or start of string
        let before_exe = &content[..exe_pos];
        let exe_start = before_exe.rfind(' ').map(|pos| pos + 1).unwrap_or(0);
        let exe_name = &content[exe_start..exe_pos + 4]; // +4 for ".exe"
        let cleaned_exe = exe_name.trim().to_string();
        
        println!("🎯 Extracted exe name: '{}'", cleaned_exe);
        return Some(cleaned_exe);
    }
    
    // If no .exe found, try to extract from common patterns
    let content_lower = content.to_lowercase();
    
    // Pattern 1: "open game with xxx.exe"
    if let Some(start) = content_lower.find("open game with ") {
        let after_with = &content[start + 15..];
        if let Some(end) = after_with.find(".exe") {
            let exe_name = &after_with[..end + 4];
            let cleaned = exe_name.trim().to_string();
            println!("🎯 Found via 'open game with' pattern: '{}'", cleaned);
            return Some(cleaned);
        }
    }
    
    // Pattern 2: "run xxx.exe"
    if let Some(start) = content_lower.find("run ") {
        let after_run = &content[start + 4..];
        if let Some(end) = after_run.find(".exe") {
            let exe_name = &after_run[..end + 4];
            let cleaned = exe_name.trim().to_string();
            println!("🎯 Found via 'run' pattern: '{}'", cleaned);
            return Some(cleaned);
        }
    }
    
    // Pattern 3: "launch xxx.exe"
    if let Some(start) = content_lower.find("launch ") {
        let after_launch = &content[start + 7..];
        if let Some(end) = after_launch.find(".exe") {
            let exe_name = &after_launch[..end + 4];
            let cleaned = exe_name.trim().to_string();
            println!("🎯 Found via 'launch' pattern: '{}'", cleaned);
            return Some(cleaned);
        }
    }
    
    println!("❌ No exe name found in note content");
    None
}

#[command]
pub async fn confirm_and_launch_game(
    executable_path: String,
) -> Result<String, String> {
    println!("Launching game from path: {}", executable_path);
    let path = Path::new(&executable_path);
    let game_dir = path.parent().ok_or("Invalid executable path")?;

    let mut command = Command::new(&executable_path);
    command.current_dir(game_dir);

    match command.spawn() {
        Ok(_) => {
            println!("Game launched successfully");
            Ok("Game launched!".to_string())
        }
        Err(e) => {
            println!("Failed to launch game: {}", e);
            Err(format!("Failed to launch game: {}", e))
        }
    }
}

#[command]
pub async fn launch_game_executable(executable_path: String, game_path: String) -> Result<(), String> {
    Command::new(&executable_path)
        .current_dir(&game_path)
        .spawn()
        .map_err(|e| format!("Failed to launch executable: {}", e))?;
    Ok(())
}

#[command]
pub async fn check_for_updates(window: tauri::Window) -> Result<(), String> {
    window.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    Ok(())
}

#[command]
pub async fn install_update(window: tauri::Window) -> Result<(), String> {
    if let Some(update) = window.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())? {
        update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn download_bypass_from_multiple_sources(
    window: &tauri::Window,
    app_id: &str,
    bypass_type: Option<u8>,
) -> Result<String, String> {
    println!("📥 Starting bypass download from multiple sources");

    let mut all_errors = Vec::new();

    // Try each source with type-specific naming
    for (source_index, source) in BYPASS_SOURCES.iter().enumerate() {
        println!("🔄 Trying source {} of {}: {}", source_index + 1, BYPASS_SOURCES.len(), source);
        
        // Generate URL based on bypass type
        let bypass_url = match bypass_type {
            Some(type_num) => format!("{}/{}_{}.zip", source, app_id, type_num),
            None => {
                // Auto-detect: try to find the first available bypass type
                let available_bypasses = detect_available_bypass_types(app_id).await;
                if let Some(first_bypass) = available_bypasses.first() {
                    if first_bypass.file_suffix == 0 {
                        format!("{}/{}.zip", source, app_id) // Legacy format
                    } else {
                        format!("{}/{}_{}.zip", source, app_id, first_bypass.file_suffix)
                    }
                } else {
                    format!("{}/{}.zip", source, app_id) // Fallback to legacy format
                }
            }
        };
        
        println!("📥 Downloading from: {}", bypass_url);

        match download_bypass_with_progress(&bypass_url, window, app_id).await {
            Ok(path) => {
                println!("✅ Download successful from source: {}", source);
                return Ok(path);
            }
            Err(e) => {
                let error_msg = format!("Source {} failed: {}", source, e);
                println!("❌ {}", error_msg);
                all_errors.push(error_msg);

                // Wait a bit before trying next source (except for last source)
                if source_index < BYPASS_SOURCES.len() - 1 {
                    println!("⏳ Waiting 2 seconds before trying next source...");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    let combined_errors = all_errors.join("; ");
    Err(format!(
        "Download failed from all {} sources. Errors: {}",
        BYPASS_SOURCES.len(),
        combined_errors
    ))
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
    install_bypass_files_with_type(extract_path, game_path, None).await
}

async fn install_bypass_files_with_type(extract_path: &str, game_path: &str, bypass_type: Option<u8>) -> Result<(), String> {
    println!("🔧 Installing bypass files");
    println!("   Source: {}", extract_path);
    println!("   Target: {}", game_path);
    if let Some(type_num) = bypass_type {
        println!("   Type: {}", type_num);
    }

    // Find the actual bypass files - they might be in a subfolder
    let bypass_source = find_bypass_files_directory(extract_path)?;
    println!("🎯 Bypass files found in: {}", bypass_source);

    // Install based on bypass type
    match bypass_type {
        Some(1) => {
            println!("📁 Installing Type 1: Files only - flat copy to root");
            copy_bypass_files_flat_impl(Path::new(&bypass_source), Path::new(game_path))
        },
        Some(2) => {
            println!("📁 Installing Type 2: Folders and files - preserve structure");
            copy_bypass_files_preserve_structure(Path::new(&bypass_source), Path::new(game_path))
        },
        Some(3) => {
            println!("📁 Installing Type 3: Advanced mixed content");
            copy_bypass_files_hybrid(Path::new(&bypass_source), Path::new(game_path))
        },
        Some(4) => {
            println!("📁 Installing Type 4: Custom installation method");
            install_bypass_type_4(&bypass_source, game_path).await
        },
        Some(5) => {
            println!("📁 Installing Type 5: Registry/config based");
            install_bypass_type_5(&bypass_source, game_path).await
        },
        _ => {
            // Auto-detect or legacy mode
            println!("📁 Auto-detecting installation method...");
            copy_bypass_files_smart(&bypass_source, game_path)
        }
    }?;
    
    println!("✅ Bypass files installed successfully");
    Ok(())
}

// Placeholder implementations for future bypass types
async fn install_bypass_type_4(extract_path: &str, game_path: &str) -> Result<(), String> {
    println!("🔧 Type 4 installation: Custom method");
    // TODO: Implement custom installation logic
    // For now, fallback to hybrid approach
    copy_bypass_files_hybrid(Path::new(extract_path), Path::new(game_path))
}

async fn install_bypass_type_5(extract_path: &str, game_path: &str) -> Result<(), String> {
    println!("🔧 Type 5 installation: Registry/config based");
    // TODO: Implement registry/config modification logic
    // For now, fallback to preserve structure
    copy_bypass_files_preserve_structure(Path::new(extract_path), Path::new(game_path))
}

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

fn copy_bypass_files_smart(src: &str, dst: &str) -> Result<(), String> {
    let src_path = Path::new(src);
    let dst_path = Path::new(dst);

    println!("🔧 Analyzing bypass structure...");
    
    // Analyze the bypass structure to determine the best copy strategy
    let structure_analysis = analyze_bypass_structure(src_path)?;
    
    match structure_analysis {
        BypassStructure::FlatFiles => {
            println!("📁 Structure: Flat files - copying directly to game root");
            copy_bypass_files_flat_impl(src_path, dst_path)
        },
        BypassStructure::FoldersWithContent => {
            println!("📁 Structure: Contains folders - preserving directory structure");
            copy_bypass_files_preserve_structure(src_path, dst_path)
        },
        BypassStructure::MixedContent => {
            println!("📁 Structure: Mixed content - using hybrid approach");
            copy_bypass_files_hybrid(src_path, dst_path)
        }
    }
}

fn analyze_bypass_structure(src_path: &Path) -> Result<BypassStructure, String> {
    let mut root_files = 0;
    let mut folders_with_content = 0;
    let mut total_files_in_subfolders = 0;
    
    println!("🔍 Analyzing directory structure: {}", src_path.display());
    
    // Read root directory
    if let Ok(entries) = fs::read_dir(src_path) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            
            if path.is_file() {
                root_files += 1;
                println!("   📄 Root file: {}", path.file_name().unwrap().to_string_lossy());
            } else if path.is_dir() {
                // Check if folder has meaningful content
                let folder_name = path.file_name().unwrap().to_string_lossy();
                let files_in_folder = count_files_in_directory(&path)?;
                
                if files_in_folder > 0 {
                    folders_with_content += 1;
                    total_files_in_subfolders += files_in_folder;
                    println!("   📁 Folder '{}' contains {} files", folder_name, files_in_folder);
                }
            }
        }
    }
    
    println!("📊 Structure analysis:");
    println!("   📄 Root files: {}", root_files);
    println!("   📁 Folders with content: {}", folders_with_content);
    println!("   📋 Files in subfolders: {}", total_files_in_subfolders);
    
    // Determine structure type
    let structure = if folders_with_content == 0 {
        BypassStructure::FlatFiles
    } else if root_files == 0 && folders_with_content > 0 {
        BypassStructure::FoldersWithContent
    } else {
        BypassStructure::MixedContent
    };
    
    println!("🎯 Detected structure: {:?}", structure);
    Ok(structure)
}

fn count_files_in_directory(dir_path: &Path) -> Result<usize, String> {
    let mut count = 0;
    for entry in WalkDir::new(dir_path) {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.path().is_file() {
            count += 1;
        }
    }
    Ok(count)
}

fn copy_bypass_files_preserve_structure(src_path: &Path, dst_path: &Path) -> Result<(), String> {
    let mut files_replaced = 0;
    let mut files_new = 0;
    let mut folders_created = 0;

    println!("📂 Installing bypass files with preserved structure");
    println!("   Source: {}", src_path.display());
    println!("   Target: {}", dst_path.display());

    for entry in WalkDir::new(src_path) {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_file_path = entry.path();
        
        // Calculate relative path from source root
        let relative_path = src_file_path.strip_prefix(src_path)
            .map_err(|e| e.to_string())?;
        
        // Skip the root directory itself
        if relative_path.as_os_str().is_empty() {
            continue;
        }
        
        let dest_path = dst_path.join(relative_path);
        
        if src_file_path.is_dir() {
            // Create directory if it doesn't exist
            if !dest_path.exists() {
                fs::create_dir_all(&dest_path).map_err(|e| e.to_string())?;
                println!("📁 Created folder: {}", relative_path.display());
                folders_created += 1;
            }
        } else if src_file_path.is_file() {
            // Ensure parent directory exists
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            
            let file_exists = dest_path.exists();
            
            // Copy the file
            fs::copy(src_file_path, &dest_path).map_err(|e| e.to_string())?;
            
            println!("📄 Installing: {}", relative_path.display());
            
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
    println!("   📁 Folders created: {}", folders_created);
    println!("   🔄 Files replaced: {}", files_replaced);
    println!("   ✅ New files added: {}", files_new);

    Ok(())
}

fn copy_bypass_files_hybrid(src_path: &Path, dst_path: &Path) -> Result<(), String> {
    let mut files_replaced = 0;
    let mut files_new = 0;
    let mut folders_created = 0;

    println!("📂 Installing bypass files with hybrid approach");
    println!("   Source: {}", src_path.display());
    println!("   Target: {}", dst_path.display());
    
    // First, copy root files directly to game root
    println!("🔧 Phase 1: Installing root files to game directory");
    if let Ok(entries) = fs::read_dir(src_path) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file() {
                let file_name = path.file_name().unwrap();
                let dest_file = dst_path.join(file_name);
                
                let file_exists = dest_file.exists();
                fs::copy(&path, &dest_file).map_err(|e| e.to_string())?;
                
                println!("📄 Installing root file: {}", file_name.to_string_lossy());
                if file_exists {
                    println!("   🔄 REPLACED existing file");
                    files_replaced += 1;
                } else {
                    println!("   ✅ Added new file");
                    files_new += 1;
                }
            }
        }
    }
    
    // Then, copy ONLY folders (not individual files) with structure preserved
    println!("🔧 Phase 2: Installing folders with preserved structure");
    if let Ok(entries) = fs::read_dir(src_path) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                let folder_name = path.file_name().unwrap();
                println!("📁 Processing folder: {}", folder_name.to_string_lossy());
                
                // Copy this folder and its contents recursively
                copy_folder_recursive(&path, &dst_path.join(folder_name), &mut files_replaced, &mut files_new, &mut folders_created)?;
            }
        }
    }
    
    println!("📊 Hybrid Installation Summary:");
    println!("   📁 Folders created: {}", folders_created);
    println!("   🔄 Files replaced: {}", files_replaced);
    println!("   ✅ New files added: {}", files_new);
    Ok(())
}

fn copy_folder_recursive(src_folder: &Path, dst_folder: &Path, files_replaced: &mut usize, files_new: &mut usize, folders_created: &mut usize) -> Result<(), String> {
    // Create destination folder if it doesn't exist
    if !dst_folder.exists() {
        fs::create_dir_all(dst_folder).map_err(|e| e.to_string())?;
        println!("   📁 Created folder: {}", dst_folder.display());
        *folders_created += 1;
    }
    
    // Copy all contents of this folder
    for entry in WalkDir::new(src_folder) {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        
        // Skip the root folder itself
        if src_path == src_folder {
            continue;
        }
        
        // Calculate relative path from src_folder
        let relative_path = src_path.strip_prefix(src_folder)
            .map_err(|e| e.to_string())?;
        let dest_path = dst_folder.join(relative_path);
        
        if src_path.is_dir() {
            // Create directory
            if !dest_path.exists() {
                fs::create_dir_all(&dest_path).map_err(|e| e.to_string())?;
                println!("   📁 Created subfolder: {}", relative_path.display());
                *folders_created += 1;
            }
        } else if src_path.is_file() {
            // Ensure parent directory exists
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            
            let file_exists = dest_path.exists();
            
            // Copy the file
            fs::copy(src_path, &dest_path).map_err(|e| e.to_string())?;
            
            println!("   📄 Installing: {}", relative_path.display());
            
            if file_exists {
                println!("      🔄 REPLACED existing file");
                *files_replaced += 1;
            } else {
                println!("      ✅ Added new file");
                *files_new += 1;
            }
        }
    }
    
    Ok(())
}

// Legacy flat copy implementation (kept for compatibility)
fn copy_bypass_files_flat_impl(src_path: &Path, dst_path: &Path) -> Result<(), String> {

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

fn cleanup_temp_files(download_path: &str, extract_path: &str) -> Result<(), String> {
    // Remove download file
    if Path::new(download_path).exists() {
        fs::remove_file(download_path).map_err(|e| format!("Failed to cleanup download: {}", e))?;
    }

    // Remove extract directory
    if Path::new(extract_path).exists() {
        fs::remove_dir_all(extract_path)
            .map_err(|e| format!("Failed to cleanup extract folder: {}", e))?;
    }
    Ok(())
}
