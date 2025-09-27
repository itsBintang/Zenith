use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DownloadType {
    Http,
    Torrent,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DownloadStatus {
    Pending,
    Active,
    Paused,
    Completed,
    Error,
    Cancelled,
    Seeding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub download_id: String,
    pub progress: f64,
    pub download_speed: u64,
    pub upload_speed: u64,
    pub total_size: u64,
    pub downloaded_size: u64,
    pub eta: Option<u64>, // seconds
    pub num_peers: u32,
    pub num_seeds: u32,
    pub status: DownloadStatus,
    pub file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub id: String,
    pub url: String,
    pub save_path: String,
    pub download_type: DownloadType,
    pub headers: Option<HashMap<String, String>>,
    pub filename: Option<String>,
    pub auto_extract: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadInfo {
    pub id: String,
    pub url: String,
    pub save_path: String,
    pub download_type: DownloadType,
    pub status: DownloadStatus,
    pub progress: DownloadProgress,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Aria2Config {
    pub host: String,
    pub port: u16,
    pub secret: Option<String>,
    pub max_concurrent_downloads: u8,
    pub max_connections_per_server: u8,
    pub split: u8,
    pub min_split_size: String,
}

impl Default for Aria2Config {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 6800,
            secret: None,
            max_concurrent_downloads: 5,
            max_connections_per_server: 4,
            split: 4,
            min_split_size: "1M".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TorrentConfig {
    pub port: u16,
    pub download_rate_limit: Option<u64>,
    pub upload_rate_limit: Option<u64>,
    pub max_connections: u16,
    pub enable_dht: bool,
    pub enable_pex: bool,
    pub enable_lsd: bool,
}

impl Default for TorrentConfig {
    fn default() -> Self {
        Self {
            port: 6881,
            download_rate_limit: None,
            upload_rate_limit: Some(1024), // 1KB/s default upload limit
            max_connections: 200,
            enable_dht: true,
            enable_pex: true,
            enable_lsd: true,
        }
    }
}
