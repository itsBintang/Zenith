use crate::download::{
    download_manager::DownloadManager,
    types::{DownloadInfo, DownloadProgress, DownloadRequest, DownloadType},
};
use anyhow::Result;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{command, AppHandle, State, Manager};
use uuid::Uuid;

pub struct DownloadManagerState {
    pub manager: Arc<RwLock<Option<Arc<DownloadManager>>>>,
}

impl DownloadManagerState {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(RwLock::new(None)),
        }
    }
}

pub fn find_aria2_binary(app_handle: &AppHandle) -> Result<PathBuf, String> {
    // Try bundled resource first
    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        let bundled_path = resource_dir.join("aria2c.exe");
        if bundled_path.exists() {
            return Ok(bundled_path);
        }
        
        // Also try without .exe extension (for externalBin)
        let bundled_path_no_ext = resource_dir.join("aria2c");
        if bundled_path_no_ext.exists() {
            return Ok(bundled_path_no_ext);
        }
    }
    
    // Fallback paths
    let fallback_paths = [
        "C:\\aria2\\aria2c.exe",
        "aria2c.exe", // In current directory
        "binaries\\aria2c.exe", // In binaries subdirectory
    ];
    
    for path in &fallback_paths {
        let path_buf = PathBuf::from(path);
        if path_buf.exists() {
            return Ok(path_buf);
        }
    }
    
    // Try to find in PATH
    if let Ok(output) = std::process::Command::new("where").arg("aria2c").output() {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout);
            let first_path = path_str.lines().next().unwrap_or("").trim();
            if !first_path.is_empty() {
                return Ok(PathBuf::from(first_path));
            }
        }
    }
    
    Err("aria2c.exe not found. Please ensure it's installed or bundled with the application.".to_string())
}

#[command]
pub async fn initialize_download_manager(
    app_handle: AppHandle,
    state: State<'_, DownloadManagerState>,
) -> Result<String, String> {
    // Try to find aria2c.exe in bundled resources or fallback paths
    let aria2_path = find_aria2_binary(&app_handle)?;
    
    let mut manager = DownloadManager::new(aria2_path)
        .map_err(|e| format!("Failed to create download manager: {}", e))?;
    
    manager.set_app_handle(app_handle);
    
    manager.initialize().await
        .map_err(|e| format!("Failed to initialize download manager: {}", e))?;
    
    {
        let mut state_manager = state.manager.write();
        *state_manager = Some(Arc::new(manager));
    }
    
    Ok("Download manager initialized successfully".to_string())
}

#[command]
pub async fn shutdown_download_manager(
    state: State<'_, DownloadManagerState>,
) -> Result<String, String> {
    let manager = {
        let mut state_manager = state.manager.write();
        state_manager.take()
    };
    
    if let Some(manager) = manager {
        manager.shutdown().await
            .map_err(|e| format!("Failed to shutdown download manager: {}", e))?;
    }
    
    Ok("Download manager shut down successfully".to_string())
}

#[command]
pub async fn start_download(
    url: String,
    save_path: String,
    filename: Option<String>,
    headers: Option<HashMap<String, String>>,
    auto_extract: Option<bool>,
    state: State<'_, DownloadManagerState>,
) -> Result<String, String> {
    let manager = {
        let manager_guard = state.manager.read();
        manager_guard.clone()
    };
    
    let manager = manager.ok_or_else(|| "Download manager not initialized".to_string())?;
    
    let download_type = DownloadManager::detect_download_type(&url);
    let download_id = Uuid::new_v4().to_string();
    
    let request = DownloadRequest {
        id: download_id.clone(),
        url,
        save_path,
        download_type,
        headers,
        filename,
        auto_extract,
    };
    
    let actual_id = manager.start_download(request).await
        .map_err(|e| format!("Failed to start download: {}", e))?;
    
    Ok(actual_id)
}

#[command]
pub async fn pause_download(
    download_id: String,
    state: State<'_, DownloadManagerState>,
) -> Result<String, String> {
    let manager = {
        let manager_guard = state.manager.read();
        manager_guard.clone()
    };
    
    let manager = manager.ok_or_else(|| "Download manager not initialized".to_string())?;
    
    manager.pause_download(&download_id).await
        .map_err(|e| format!("Failed to pause download: {}", e))?;
    
    Ok(format!("Download {} paused successfully", download_id))
}

#[command]
pub async fn resume_download(
    download_id: String,
    state: State<'_, DownloadManagerState>,
) -> Result<String, String> {
    let manager = {
        let manager_guard = state.manager.read();
        manager_guard.clone()
    };
    
    let manager = manager.ok_or_else(|| "Download manager not initialized".to_string())?;
    
    manager.resume_download(&download_id).await
        .map_err(|e| format!("Failed to resume download: {}", e))?;
    
    Ok(format!("Download {} resumed successfully", download_id))
}

#[command]
pub async fn cancel_download(
    download_id: String,
    state: State<'_, DownloadManagerState>,
) -> Result<String, String> {
    let manager = {
        let manager_guard = state.manager.read();
        manager_guard.clone()
    };
    
    let manager = manager.ok_or_else(|| "Download manager not initialized".to_string())?;
    
    manager.cancel_download(&download_id).await
        .map_err(|e| format!("Failed to cancel download: {}", e))?;
    
    Ok(format!("Download {} cancelled successfully", download_id))
}

#[command]
pub async fn get_download_progress(
    download_id: String,
    state: State<'_, DownloadManagerState>,
) -> Result<DownloadProgress, String> {
    let manager = {
        let manager_guard = state.manager.read();
        manager_guard.clone()
    };
    
    let manager = manager.ok_or_else(|| "Download manager not initialized".to_string())?;
    
    let progress = manager.get_download_progress(&download_id).await
        .map_err(|e| format!("Failed to get download progress: {}", e))?;
    
    Ok(progress)
}

#[command]
pub async fn get_all_downloads(
    state: State<'_, DownloadManagerState>,
) -> Result<Vec<DownloadInfo>, String> {
    let manager = {
        let manager_guard = state.manager.read();
        manager_guard.clone()
    };
    
    let manager = manager.ok_or_else(|| "Download manager not initialized".to_string())?;
    
    let downloads = manager.get_all_downloads().await
        .map_err(|e| format!("Failed to get downloads: {}", e))?;
    
    Ok(downloads)
}

#[command]
pub async fn get_active_downloads(
    state: State<'_, DownloadManagerState>,
) -> Result<Vec<DownloadProgress>, String> {
    let manager = {
        let manager_guard = state.manager.read();
        manager_guard.clone()
    };
    
    let manager = manager.ok_or_else(|| "Download manager not initialized".to_string())?;
    
    let downloads = manager.get_active_downloads().await
        .map_err(|e| format!("Failed to get active downloads: {}", e))?;
    
    Ok(downloads)
}

#[command]
pub async fn is_download_manager_ready(
    state: State<'_, DownloadManagerState>,
) -> Result<bool, String> {
    let manager = {
        let manager_guard = state.manager.read();
        manager_guard.clone()
    };
    
    if let Some(manager) = manager {
        Ok(manager.is_aria2_running())
    } else {
        Ok(false)
    }
}

#[command]
pub async fn detect_url_type(url: String) -> Result<String, String> {
    let download_type = DownloadManager::detect_download_type(&url);
    match download_type {
        DownloadType::Http => Ok("http".to_string()),
        DownloadType::Torrent => Ok("torrent".to_string()),
    }
}
