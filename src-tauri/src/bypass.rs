use serde::{Deserialize, Serialize};
use crate::steam_utils::{find_game_folder_from_acf, find_steam_installation_path};
use crate::download::{DownloadManagerState};
use crate::download::types::{DownloadRequest, DownloadType, DownloadStatus};
use crate::database::history_commands::{add_download_to_history, update_download_history_completion};
use std::fs::{self, File};
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::command;
use tauri::{Emitter, Manager, State};
use walkdir::WalkDir;
use zip::ZipArchive;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BypassProgress {
    step: String,
    progress: f64,
    app_id: String,
    download_id: Option<String>,
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

// Removed BYPASS_SOURCES - now using URLs directly from JSON data

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
pub async fn install_bypass(app_id: String, manual_game_path: Option<String>, window: tauri::Window, download_state: State<'_, DownloadManagerState>) -> Result<BypassResult, String> {
    install_bypass_with_type(app_id, None, manual_game_path, window, download_state).await
}

#[command]
pub async fn install_bypass_with_type(app_id: String, _bypass_type: Option<u8>, manual_game_path: Option<String>, window: tauri::Window, download_state: State<'_, DownloadManagerState>) -> Result<BypassResult, String> {
    // Check if bypass is already installed
    let is_reinstall = check_bypass_installed(&app_id).await.unwrap_or(false);

    if is_reinstall {
        println!("🔄 Starting bypass REINSTALLATION for AppID: {}", app_id);
        println!("   (Previous installation detected - will overwrite)");
    } else {
        println!("🚀 Starting UNIVERSAL bypass installation for AppID: {}", app_id);
    }
    println!("================================");

    let emit_progress = |step: &str, progress: f64, download_id: Option<String>| {
        println!("📊 Progress: {:.1}% - {}", progress, step);
        let _ = window.emit(
            "bypass_progress",
            BypassProgress {
                step: step.to_string(),
                progress,
                app_id: app_id.clone(),
                download_id,
            },
        );
    };

    let action_word = if is_reinstall {
        "reinstallation"
    } else {
        "installation"
    };
    emit_progress(&format!("Initializing bypass {}...", action_word), 0.0, None);

    // Step 1: Detect Steam installation
    emit_progress("Detecting Steam installation...", 10.0, None);
    let steam_path = find_steam_installation_path()
        .map_err(|e| format!("Steam installation not found: {}", e))?;

    // Step 2: Validate game installation
    emit_progress("Validating game installation...", 20.0, None);
    
    let game_path = if let Some(manual_path) = manual_game_path {
        println!("📁 Using manual game path: {}", manual_path);
        manual_path
    } else {
        let game_folder = find_game_folder_from_acf(&app_id, &steam_path)
            .await
            .ok_or_else(|| "Game not found in Steam library or not fully installed".to_string())?;
        println!("📁 Found game folder: {}", game_folder);
        
        let path = format!("{}/steamapps/common/{}", steam_path, game_folder);
        println!("🎯 Auto-detected game path: {}", path);
        path
    };

    if !Path::new(&game_path).exists() {
        let error_msg = format!("Game directory does not exist: {}", game_path);
        println!("❌ {}", error_msg);
        return Err(error_msg);
    }

    println!("✅ Game directory validated successfully");

    // Step 3: Download bypass using aria2c
    emit_progress("Downloading bypass files with aria2c...", 30.0, None);
    
    let (download_path, download_id) = download_bypass_from_json_with_aria2(&window, &app_id, &download_state)
        .await
        .map_err(|e| {
            println!("❌ aria2c download completely failed: {}", e);
            format!("Failed to download bypass: {}", e)
        })?;

    // Step 4: Extract bypass
    emit_progress("Extracting bypass files...", 60.0, None);
    let extract_path = extract_bypass(&download_path)
        .await
        .map_err(|e| format!("Failed to extract bypass: {}", e))?;

    // Step 5: Install bypass files - UNIVERSAL METHOD (pure copy structure)
    emit_progress("Installing bypass to game directory...", 85.0, None);
    install_bypass_files_universal(&extract_path, &game_path)
        .await
        .map_err(|e| format!("Failed to install bypass: {}", e))?;

    // Step 6: Cleanup
    emit_progress("Finalizing installation...", 95.0, None);
    cleanup_temp_files(&download_path, &extract_path)?;
    println!("🧹 Cleaned up temporary files");

    // Mark bypass as installed
    let installed_marker = format!("{}/bypass_installed.txt", game_path);
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S");
    let marker_content = format!(
        "Bypass installed by Zenith (Universal Method)\nAppID: {}\nInstalled: {}\nGame Path: {}",
        app_id, timestamp, game_path
    );
    let _ = fs::write(&installed_marker, marker_content);
    println!("📝 Created installation marker");

    let final_message = if is_reinstall {
        "Bypass reinstalled successfully!"
    } else {
        "Bypass installed successfully!"
    };

    emit_progress(final_message, 100.0, None);

    println!("🎉 Bypass {} completed successfully!", action_word);
    println!("📁 Game directory: {}", game_path);
    println!("================================");

    // Don't show launch popup - just finish bypass installation
    Ok(BypassResult {
        success: true,
        message: final_message.to_string(),
        should_launch: false,  // Don't show popup
        game_executable_path: None, // No need to pass path
    })
}

#[command]
pub async fn check_bypass_installed_command(app_id: String) -> Result<bool, String> {
    check_bypass_installed(&app_id).await
}

#[derive(Debug, Serialize)]
pub struct GameInstallationInfo {
    pub install_path: String,
    pub steam_path: String,
    pub game_folder: String,
}

#[command]
pub async fn get_game_installation_info(app_id: String) -> Result<GameInstallationInfo, String> {
    let steam_path = find_steam_installation_path().map_err(|e| e.to_string())?;
    
    let game_folder = match find_game_folder_from_acf(&app_id, &steam_path).await {
        Some(folder) => folder,
        None => return Err(format!("Game not found with app_id: {}", app_id)),
    };
    
    let install_path = format!("{}/steamapps/common/{}", steam_path, game_folder);
    
    if !std::path::Path::new(&install_path).exists() {
        return Err(format!("Game directory does not exist: {}", install_path));
    }
    
    Ok(GameInstallationInfo {
        install_path,
        steam_path,
        game_folder,
    })
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
    game_name: String,
) -> Result<String, String> {
    println!("🎮 User confirmed to launch game: {}", game_name);
    println!("📁 Executable path: {}", executable_path);

    launch_game_executable(executable_path).await
}

#[command]
pub async fn launch_game_executable(executable_path: String) -> Result<String, String> {
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

// ====================== BYPASS DOWNLOAD & INSTALL FUNCTIONS ======================


async fn download_bypass_from_json_with_aria2(
    window: &tauri::Window,
    app_id: &str,
    download_state: &State<'_, DownloadManagerState>,
) -> Result<(String, String), String> {
    println!("📥 Starting bypass download using aria2c from URLs in JSON");

    // Get bypass game data from cache
    let bypass_game = crate::database::cache_service::SQLITE_CACHE_SERVICE
        .get_bypass_game(app_id)
        .await
        .map_err(|e| format!("Failed to get bypass game data: {}", e))?;

    let bypass_game = bypass_game.ok_or_else(|| {
        format!("No bypass data found for game ID: {}", app_id)
    })?;

    if bypass_game.bypasses.is_empty() {
        return Err(format!("No bypass URLs found for game: {}", bypass_game.name));
    }

    println!("🎯 Found {} bypass URLs for game: {}", bypass_game.bypasses.len(), bypass_game.name);

    let mut all_errors = Vec::new();

    // Try each bypass URL from JSON using aria2c
    for (index, bypass) in bypass_game.bypasses.iter().enumerate() {
        println!("🔄 Trying bypass {} of {} with aria2c: {}", index + 1, bypass_game.bypasses.len(), bypass.url);

        match download_bypass_with_aria2(&bypass.url, window, app_id, download_state).await {
            Ok((path, download_id)) => {
                println!("✅ aria2c download successful from bypass URL: {}", bypass.url);
                return Ok((path, download_id));
            }
            Err(e) => {
                let error_msg = format!("Bypass URL {} failed: {}", bypass.url, e);
                println!("❌ {}", error_msg);
                all_errors.push(error_msg);

                // Wait a bit before trying next URL (except for last URL)
                if index < bypass_game.bypasses.len() - 1 {
                    println!("⏳ Waiting 2 seconds before trying next URL...");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    let combined_errors = all_errors.join("; ");
    Err(format!(
        "Download failed from all {} bypass URLs for game {}. Errors: {}",
        bypass_game.bypasses.len(),
        bypass_game.name,
        combined_errors
    ))
}

async fn download_bypass_with_aria2(
    bypass_url: &str,
    window: &tauri::Window,
    app_id: &str,
    download_state: &State<'_, DownloadManagerState>,
) -> Result<(String, String), String> {
    println!("📥 Starting aria2c download from: {}", bypass_url);

    // Get download manager, initialize if needed
    let manager_option = {
        let manager_guard = download_state.manager.read();
        manager_guard.clone()
    };

    let manager = match manager_option {
        Some(manager) => manager,
        None => {
            // Auto-initialize download manager for bypass
            println!("🔧 Download manager not initialized, auto-initializing for bypass...");
            
            // Initialize download manager
            let aria2_path = crate::download::commands::find_aria2_binary(&window.app_handle())
                .map_err(|e| format!("Failed to find aria2 binary: {}", e))?;
            
            let mut new_manager = crate::download::download_manager::DownloadManager::new(aria2_path)
                .map_err(|e| format!("Failed to create download manager: {}", e))?;
            
            new_manager.set_app_handle(window.app_handle().clone());
            new_manager.initialize().await
                .map_err(|e| format!("Failed to initialize download manager: {}", e))?;
            
            let manager_arc = std::sync::Arc::new(new_manager);
            
            // Store in state
            {
                let mut state_manager = download_state.manager.write();
                *state_manager = Some(manager_arc.clone());
            }
            
            println!("✅ Download manager auto-initialized successfully for bypass");
            manager_arc
        }
    };

    // Create temp directory for bypass download
    let temp_dir = std::env::temp_dir();
    let filename = format!(
        "bypass_{}_{}_{}.zip",
        app_id,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        Uuid::new_v4().simple()
    );
    
    let download_path = temp_dir.join(&filename);

    // Create download request
    let download_request = DownloadRequest {
        id: Uuid::new_v4().to_string(),
        url: bypass_url.to_string(),
        save_path: temp_dir.to_string_lossy().to_string(),
        download_type: DownloadType::Http,
        headers: None,
        filename: Some(filename.clone()),
        auto_extract: Some(false),
    };

    // Start download with aria2c
    let download_id = manager.start_download(download_request)
        .await
        .map_err(|e| format!("Failed to start aria2c download: {}", e))?;

    println!("🚀 Started aria2c download with ID: {}", download_id);

    // Add to download history
    let history_result = add_download_to_history(
        download_id.clone(),
        "bypass".to_string(),
        "bypass_installation".to_string(),
        bypass_url.to_string(),
        download_path.to_string_lossy().to_string(),
        Some(app_id.to_string()),
        None, // We'll get game name later if needed
        None, // No original request JSON for bypass
    ).await;

    if let Err(e) = history_result {
        println!("⚠️ Failed to add download to history: {}", e);
        // Don't fail the download, just log the error
    } else {
        println!("📝 Added bypass download to history");
    }

    // Monitor download progress
    let mut last_progress_time = std::time::Instant::now();
    let mut completed = false;

    for _ in 0..300 { // Max 5 minutes timeout (300 * 1 second)
        tokio::time::sleep(Duration::from_secs(1)).await;

        match manager.get_download_progress(&download_id).await {
            Ok(progress) => {
                // Update progress every 2 seconds to avoid spam
                if last_progress_time.elapsed() >= Duration::from_secs(2) {
                    let bypass_progress = 30.0 + (progress.progress * 30.0); // 30% to 60% of total progress
                    
                    let speed_mbps = progress.download_speed as f64 / 1_048_576.0;
                    let step_message = if progress.total_size > 0 {
                        format!(
                            "Downloading with aria2c... {:.1} MB / {:.1} MB ({:.1} MB/s)",
                            progress.downloaded_size as f64 / 1_048_576.0,
                            progress.total_size as f64 / 1_048_576.0,
                            speed_mbps
                        )
                    } else {
                        format!(
                            "Downloading with aria2c... {:.1} MB ({:.1} MB/s)",
                            progress.downloaded_size as f64 / 1_048_576.0,
                            speed_mbps
                        )
                    };

                    let _ = window.emit(
                        "bypass_progress",
                        BypassProgress {
                            step: step_message,
                            progress: bypass_progress,
                            app_id: app_id.to_string(),
                            download_id: Some(download_id.clone()),
                        },
                    );

                    last_progress_time = std::time::Instant::now();
                }

                match progress.status {
                    DownloadStatus::Completed => {
                        completed = true;
                        break;
                    }
                    DownloadStatus::Error => {
                        return Err("Download failed with error status".to_string());
                    }
                    DownloadStatus::Cancelled => {
                        return Err("Download was cancelled".to_string());
                    }
                    _ => {
                        // Continue monitoring
                    }
                }
            }
            Err(e) => {
                println!("⚠️ Failed to get progress: {}", e);
                // Continue trying, maybe download isn't started yet
            }
        }
    }

    if !completed {
        // Update history as cancelled/failed
        let _ = update_download_history_completion(
            download_id.clone(),
            "failed".to_string(),
            0.0,
            0,
            300, // 5 minutes timeout
            None,
            Some("Download timeout after 5 minutes".to_string()),
        ).await;
        
        // Cleanup incomplete download
        let _ = manager.cancel_download(&download_id).await;
        return Err("Download timeout after 5 minutes".to_string());
    }

    // Verify file exists
    if !download_path.exists() {
        // Update history as failed
        let _ = update_download_history_completion(
            download_id.clone(),
            "failed".to_string(),
            0.9, // Almost complete but file missing
            0,
            300,
            None,
            Some("Download completed but file not found".to_string()),
        ).await;
        
        return Err(format!("Download completed but file not found: {}", download_path.display()));
    }

    // Get file size for history
    let file_size = std::fs::metadata(&download_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    // Update history as completed
    let _ = update_download_history_completion(
        download_id.clone(),
        "completed".to_string(),
        1.0, // 100% complete
        0,   // We don't track speed in bypass currently
        300, // Approximate time - could be improved
        Some(file_size),
        None,
    ).await;

    println!("✅ aria2c download completed: {}", download_path.display());
    println!("📝 Updated download history as completed");
    Ok((download_path.to_string_lossy().to_string(), download_id))
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

// UNIVERSAL BYPASS INSTALLATION - Pure copy structure as-is from ZIP
async fn install_bypass_files_universal(extract_path: &str, game_path: &str) -> Result<(), String> {
    println!("🔧 Installing bypass files - UNIVERSAL METHOD");
    println!("   Source: {}", extract_path);
    println!("   Target: {}", game_path);
    println!("   Method: Pure copy structure (no modification)");

    let extract_dir = Path::new(extract_path);
    let game_dir = Path::new(game_path);

    if !extract_dir.exists() {
        return Err("Extract directory does not exist".to_string());
    }

    if !game_dir.exists() {
        return Err("Game directory does not exist".to_string());
    }

    // Copy everything from extract directory to game directory - preserve structure exactly
    copy_dir_contents_recursive(extract_dir, game_dir)?;
    
    // Check if note.txt or note file was installed and log it
    let note_txt_path = format!("{}/note.txt", game_path);
    let note_path = format!("{}/note", game_path);
    
    if Path::new(&note_txt_path).exists() {
        println!("📝 Note file detected: note.txt");
        if let Ok(content) = fs::read_to_string(&note_txt_path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                println!("📋 Note content: {}", trimmed);
                if let Some(exe_name) = extract_recommended_exe(trimmed) {
                    println!("🎯 Recommended executable: {}", exe_name);
                }
            }
        }
    } else if Path::new(&note_path).exists() {
        println!("📝 Note file detected: note");
        if let Ok(content) = fs::read_to_string(&note_path) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                println!("📋 Note content: {}", trimmed);
                if let Some(exe_name) = extract_recommended_exe(trimmed) {
                    println!("🎯 Recommended executable: {}", exe_name);
                }
            }
        }
            } else {
        println!("📝 No note file found in bypass");
    }
    
    println!("✅ Bypass files installed successfully with UNIVERSAL method");
    Ok(())
}

// Copy directory contents with preserved structure - exactly as extracted from ZIP
fn copy_dir_contents_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    println!("📂 Copying contents from: {}", src.display());
    println!("📂 Copying contents to: {}", dst.display());
    println!("📝 Method: PRESERVE STRUCTURE - Copy all folders and files as-is");

    let mut files_copied = 0;
    let mut dirs_created = 0;

    // Walk through ALL contents in the source directory
    for entry in WalkDir::new(src) {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let src_path = entry.path();
        
        // Skip the root source directory itself
        if src_path == src {
            continue;
        }
        
        // Calculate relative path from source
        let relative_path = src_path.strip_prefix(src)
            .map_err(|e| format!("Failed to get relative path: {}", e))?;
        
        let dst_path = dst.join(relative_path);

        if src_path.is_dir() {
            // Create directory if it doesn't exist
            if !dst_path.exists() {
                fs::create_dir_all(&dst_path)
                    .map_err(|e| format!("Failed to create directory {}: {}", dst_path.display(), e))?;
                dirs_created += 1;
                println!("📁 Created directory: {}", relative_path.display());
            }
        } else if src_path.is_file() {
            // Ensure parent directory exists
            if let Some(parent) = dst_path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create parent directory: {}", e))?;
                }
            }

            // Copy file with preserved structure
            fs::copy(src_path, &dst_path)
                .map_err(|e| format!("Failed to copy file {}: {}", src_path.display(), e))?;
            
            files_copied += 1;
            let file_size = src_path.metadata().map(|m| m.len()).unwrap_or(0);
            println!("📄 Copied file: {} ({:.1} KB)", 
                    relative_path.display(), 
                    file_size as f64 / 1024.0);
        }
    }

    println!("📊 Structure-preserving copy completed:");
    println!("   📁 Directories created: {}", dirs_created);
    println!("   📄 Files copied: {}", files_copied);

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
