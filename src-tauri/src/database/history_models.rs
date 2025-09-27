use chrono::{DateTime, Utc};
use rusqlite::{Row, Result as SqliteResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadHistoryEntry {
    pub id: Option<i64>,
    pub download_id: String,
    pub download_type: String,      // 'bypass' or 'regular'
    pub source_type: String,        // 'bypass', 'manual', 'game_download', etc.
    
    // Download Details
    pub url: String,
    pub file_name: Option<String>,
    pub file_size: i64,
    pub save_path: String,
    
    // Game/Bypass specific
    pub app_id: Option<String>,
    pub game_name: Option<String>,
    
    // Progress & Status
    pub final_progress: f64,        // 0.0 to 1.0
    pub download_speed_avg: i64,    // bytes/s
    pub total_time_seconds: i64,
    
    // Status & Result
    pub status: String,             // 'completed', 'cancelled', 'failed'
    pub error_message: Option<String>,
    
    // Timestamps (stored as Unix timestamps)
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub created_at: i64,
    
    // Metadata
    pub user_agent: Option<String>,
    pub headers: Option<String>,    // JSON string
    
    // Re-download capability
    pub is_redownloadable: bool,
    pub original_request: Option<String>, // JSON of original request
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadHistorySummary {
    pub id: i64,
    pub download_type: String,
    pub source_type: String,
    pub file_name: Option<String>,
    pub file_size_mb: f64,
    pub app_id: Option<String>,
    pub game_name: Option<String>,
    pub status: String,
    pub progress_percent: f64,
    pub avg_speed_mbps: f64,
    pub total_time_seconds: i64,
    pub started_at: i64,
    pub completed_at: Option<i64>,
    pub is_redownloadable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryStats {
    pub total_downloads: i64,
    pub completed_downloads: i64,
    pub failed_downloads: i64,
    pub total_data_downloaded_gb: f64,
    pub bypass_downloads: i64,
    pub regular_downloads: i64,
    pub avg_download_speed_mbps: f64,
    pub total_download_time_hours: f64,
}

impl DownloadHistoryEntry {
    pub fn new(
        download_id: String,
        download_type: String,
        source_type: String,
        url: String,
        save_path: String,
    ) -> Self {
        let now = Utc::now().timestamp();
        Self {
            id: None,
            download_id,
            download_type,
            source_type,
            url,
            file_name: None,
            file_size: 0,
            save_path,
            app_id: None,
            game_name: None,
            final_progress: 0.0,
            download_speed_avg: 0,
            total_time_seconds: 0,
            status: "started".to_string(),
            error_message: None,
            started_at: now,
            completed_at: None,
            created_at: now,
            user_agent: None,
            headers: None,
            is_redownloadable: true,
            original_request: None,
        }
    }

    pub fn mark_completed(&mut self, final_progress: f64, avg_speed: i64, total_time: i64) {
        self.final_progress = final_progress;
        self.download_speed_avg = avg_speed;
        self.total_time_seconds = total_time;
        self.status = "completed".to_string();
        self.completed_at = Some(Utc::now().timestamp());
    }

    pub fn mark_failed(&mut self, error_message: String) {
        self.status = "failed".to_string();
        self.error_message = Some(error_message);
        self.completed_at = Some(Utc::now().timestamp());
    }

    pub fn mark_cancelled(&mut self) {
        self.status = "cancelled".to_string();
        self.completed_at = Some(Utc::now().timestamp());
    }

    pub fn from_row(row: &Row) -> SqliteResult<Self> {
        Ok(Self {
            id: Some(row.get("id")?),
            download_id: row.get("download_id")?,
            download_type: row.get("download_type")?,
            source_type: row.get("source_type")?,
            url: row.get("url")?,
            file_name: row.get("file_name")?,
            file_size: row.get("file_size")?,
            save_path: row.get("save_path")?,
            app_id: row.get("app_id")?,
            game_name: row.get("game_name")?,
            final_progress: row.get("final_progress")?,
            download_speed_avg: row.get("download_speed_avg")?,
            total_time_seconds: row.get("total_time_seconds")?,
            status: row.get("status")?,
            error_message: row.get("error_message")?,
            started_at: row.get("started_at")?,
            completed_at: row.get("completed_at")?,
            created_at: row.get("created_at")?,
            user_agent: row.get("user_agent")?,
            headers: row.get("headers")?,
            is_redownloadable: row.get("is_redownloadable")?,
            original_request: row.get("original_request")?,
        })
    }
}

impl DownloadHistorySummary {
    pub fn from_row(row: &Row) -> SqliteResult<Self> {
        Ok(Self {
            id: row.get("id")?,
            download_type: row.get("download_type")?,
            source_type: row.get("source_type")?,
            file_name: row.get("file_name")?,
            file_size_mb: row.get("file_size_mb")?,
            app_id: row.get("app_id")?,
            game_name: row.get("game_name")?,
            status: row.get("status")?,
            progress_percent: row.get("progress_percent")?,
            avg_speed_mbps: row.get("avg_speed_mbps")?,
            total_time_seconds: row.get("total_time_seconds")?,
            started_at: row.get("started_at")?,
            completed_at: row.get("completed_at")?,
            is_redownloadable: row.get("is_redownloadable")?,
        })
    }
}
