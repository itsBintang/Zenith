use tauri::command;
use crate::database::{history_models::*, history_operations::DownloadHistoryOperations, DatabaseManager};
use std::path::PathBuf;
use std::fs;

fn get_history_db_path() -> Result<PathBuf, anyhow::Error> {
    // Use the same database path as the main cache database
    let cache_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zenith-launcher")
        .join("cache");
    
    fs::create_dir_all(&cache_dir)?;
    Ok(cache_dir.join("games.db"))
}

/// Get download history with pagination and filtering
#[command]
pub async fn get_download_history(
    limit: Option<u32>,
    offset: Option<u32>,
    filter_type: Option<String>, // 'bypass', 'regular', or None for all
) -> Result<Vec<DownloadHistorySummary>, String> {
    let db_path = get_history_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    let filter = filter_type.as_deref();
    
    db_manager.with_connection(|conn| {
        DownloadHistoryOperations::get_history_summary(conn, limit, offset, filter)
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))
    }).map_err(|e| format!("Failed to get download history: {}", e))
}

/// Get download history statistics
#[command]
pub async fn get_download_history_stats() -> Result<HistoryStats, String> {
    let db_path = get_history_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    db_manager.with_connection(|conn| {
        DownloadHistoryOperations::get_history_stats(conn)
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))
    }).map_err(|e| format!("Failed to get history stats: {}", e))
}

/// Search download history
#[command]
pub async fn search_download_history(
    search_term: String,
    limit: Option<u32>,
) -> Result<Vec<DownloadHistorySummary>, String> {
    if search_term.trim().is_empty() {
        return Ok(Vec::new());
    }
    
    let db_path = get_history_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    db_manager.with_connection(|conn| {
        DownloadHistoryOperations::search_history(conn, &search_term, limit)
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))
    }).map_err(|e| format!("Failed to search history: {}", e))
}

/// Get full download history entry by ID
#[command]
pub async fn get_download_history_entry(id: i64) -> Result<Option<DownloadHistoryEntry>, String> {
    let db_path = get_history_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    db_manager.with_connection(|conn| {
        DownloadHistoryOperations::get_download_by_id(conn, id)
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))
    }).map_err(|e| format!("Failed to get download entry: {}", e))
}

/// Delete download history entry
#[command]
pub async fn delete_download_history_entry(id: i64) -> Result<bool, String> {
    let db_path = get_history_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    db_manager.with_connection(|conn| {
        DownloadHistoryOperations::delete_history_entry(conn, id)
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))
    }).map_err(|e| format!("Failed to delete history entry: {}", e))
}

/// Clear download history (with optional filter)
#[command]
pub async fn clear_download_history(filter_type: Option<String>) -> Result<u32, String> {
    let db_path = get_history_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    let filter = filter_type.as_deref();
    
    db_manager.with_connection(|conn| {
        DownloadHistoryOperations::clear_history(conn, filter)
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))
    }).map_err(|e| format!("Failed to clear history: {}", e))
}

/// Re-download from history entry
#[command]
pub async fn redownload_from_history(
    history_id: i64,
    new_save_path: Option<String>,
) -> Result<String, String> {
    let db_path = get_history_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    // Get the original download entry
    let entry = db_manager.with_connection(|conn| {
        DownloadHistoryOperations::get_download_by_id(conn, history_id)
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))
    }).map_err(|e| format!("Failed to get history entry: {}", e))?
        .ok_or("History entry not found")?;
    
    if !entry.is_redownloadable {
        return Err("This download is not re-downloadable".to_string());
    }
    
    // Parse original request if available
    let save_path = new_save_path.unwrap_or(entry.save_path);
    
    // For now, return the information needed to restart download
    // In a full implementation, this would integrate with the download manager
    Ok(format!(
        "Ready to re-download: {} to {}. Original URL: {}",
        entry.file_name.unwrap_or("Unknown file".to_string()),
        save_path,
        entry.url
    ))
}

/// Debug command to check database tables
#[command]
pub async fn debug_history_database() -> Result<String, String> {
    let db_path = get_history_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    db_manager.with_connection(|conn| {
        // Get schema version
        let schema_version: i32 = conn.query_row(
            "SELECT value FROM cache_metadata WHERE key = 'schema_version'",
            [],
            |row| {
                let value: String = row.get(0)?;
                Ok(value.parse::<i32>().unwrap_or(0))
            },
        ).unwrap_or(0);

        // Check if download_history table exists
        let table_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='download_history'",
            [],
            |row| Ok(row.get::<_, i32>(0)? > 0)
        ).unwrap_or(false);

        // Get all table names
        let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'")?;
        let table_names: Vec<String> = stmt.query_map([], |row| {
            Ok(row.get::<_, String>(0)?)
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(format!(
            "Database Debug Info:\n- Schema Version: {}\n- History Table Exists: {}\n- All Tables: {}\n- DB Path: {}",
            schema_version,
            table_exists,
            table_names.join(", "),
            db_manager.db_path().display()
        ))
    }).map_err(|e| format!("Failed to debug database: {}", e))
}

/// Add download to history (internal function for integration)
pub async fn add_download_to_history(
    download_id: String,
    download_type: String,
    source_type: String,
    url: String,
    save_path: String,
    app_id: Option<String>,
    game_name: Option<String>,
    original_request: Option<String>,
) -> Result<i64, String> {
    let db_path = get_history_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    let mut entry = DownloadHistoryEntry::new(download_id, download_type, source_type, url, save_path);
    entry.app_id = app_id;
    entry.game_name = game_name;
    entry.original_request = original_request;
    
    db_manager.with_connection(|conn| {
        DownloadHistoryOperations::add_download(conn, &entry)
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))
    }).map_err(|e| format!("Failed to add download to history: {}", e))
}

/// Update download completion in history (internal function for integration)
pub async fn update_download_history_completion(
    download_id: String,
    status: String,
    final_progress: f64,
    avg_speed: i64,
    total_time: i64,
    file_size: Option<i64>,
    error_message: Option<String>,
) -> Result<(), String> {
    let db_path = get_history_db_path().map_err(|e| e.to_string())?;
    let db_manager = DatabaseManager::new(db_path).map_err(|e| e.to_string())?;
    
    db_manager.with_connection(|conn| {
        DownloadHistoryOperations::update_download_completion(
            conn,
            &download_id,
            &status,
            final_progress,
            avg_speed,
            total_time,
            file_size,
            error_message.as_deref(),
        ).map_err(|e| anyhow::anyhow!("Database error: {}", e))
    }).map_err(|e| format!("Failed to update download history: {}", e))
}
