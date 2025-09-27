use crate::download::{
    aria2_service::Aria2Service,
    torrent_downloader::TorrentDownloader,
    types::{DownloadInfo, DownloadProgress, DownloadRequest, DownloadStatus, DownloadType, Aria2Config, TorrentConfig},
};
use crate::database::history_commands::{add_download_to_history, update_download_history_completion};
use anyhow::{anyhow, Result};
use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct DownloadManager {
    aria2: Arc<Aria2Service>,
    torrent: Arc<TorrentDownloader>,
    downloads: Arc<RwLock<HashMap<String, DownloadInfo>>>,
    app_handle: Option<AppHandle>,
    progress_sender: Option<mpsc::UnboundedSender<DownloadProgress>>,
}

impl DownloadManager {
    pub fn new(aria2_binary_path: PathBuf) -> Result<Self> {
        let _aria2_config = Aria2Config::default();
        let torrent_config = TorrentConfig::default();
        
        let aria2 = Arc::new(Aria2Service::new(aria2_binary_path)?);
        let torrent = Arc::new(TorrentDownloader::new(torrent_config));
        
        Ok(Self {
            aria2,
            torrent,
            downloads: Arc::new(RwLock::new(HashMap::new())),
            app_handle: None,
            progress_sender: None,
        })
    }

    pub fn set_app_handle(&mut self, app_handle: AppHandle) {
        self.app_handle = Some(app_handle);
    }

    pub fn set_progress_sender(&mut self, sender: mpsc::UnboundedSender<DownloadProgress>) {
        self.progress_sender = Some(sender);
    }

    pub async fn initialize(&self) -> Result<()> {
        println!("Initializing download manager...");
        
        // Start aria2c
        self.aria2.start().await?;
        
        // Start torrent session
        self.torrent.start_session().await?;
        
        // Start progress monitoring
        self.start_progress_monitoring().await;
        
        println!("Download manager initialized successfully");
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        println!("Shutting down download manager...");
        
        // Stop aria2c
        self.aria2.stop().await?;
        
        println!("Download manager shut down successfully");
        Ok(())
    }

    pub async fn start_download(&self, request: DownloadRequest) -> Result<String> {
        let download_id = request.id.clone();
        
        println!("Starting download: {} ({})", download_id, request.url);
        
        let actual_download_id = match request.download_type {
            DownloadType::Http => {
                self.aria2.add_download(
                    &request.url,
                    &request.save_path,
                    request.filename.as_deref(),
                    request.headers.as_ref(),
                ).await?
            }
            DownloadType::Torrent => {
                if request.url.starts_with("magnet:") {
                    self.torrent.add_magnet(&request.url, PathBuf::from(&request.save_path)).await?
                } else {
                    // For torrent files, we'd need to download the .torrent file first
                    // For now, assume it's a magnet link
                    return Err(anyhow!("Torrent file downloads not yet implemented"));
                }
            }
        };

        // Clone values before move
        let url_clone = request.url.clone();
        let save_path_clone = request.save_path.clone();
        let download_type_clone = request.download_type.clone();
        let filename_clone = request.filename.clone();
        let request_json = serde_json::to_string(&request).unwrap_or_default();

        let download_info = DownloadInfo {
            id: download_id.clone(),
            url: request.url,
            save_path: request.save_path,
            download_type: request.download_type,
            status: DownloadStatus::Active,
            progress: DownloadProgress {
                download_id: actual_download_id.clone(),
                progress: 0.0,
                download_speed: 0,
                upload_speed: 0,
                total_size: 0,
                downloaded_size: 0,
                eta: None,
                num_peers: 0,
                num_seeds: 0,
                status: DownloadStatus::Active,
                file_name: request.filename,
            },
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        {
            let mut downloads = self.downloads.write();
            // Use the actual download ID from aria2/torrent as the key
            downloads.insert(actual_download_id.clone(), download_info);
        }

        // Add to download history
        let download_type_str = match download_type_clone {
            DownloadType::Http => "http",
            DownloadType::Torrent => "torrent",
        };

        let history_result = add_download_to_history(
            actual_download_id.clone(),
            download_type_str.to_string(),
            "manual".to_string(), // Regular downloads are manual
            url_clone,
            save_path_clone,
            None, // No app_id for regular downloads
            None, // No game name for regular downloads
            Some(request_json),
        ).await;

        if let Err(e) = history_result {
            println!("⚠️ Failed to add download to history: {}", e);
            // Don't fail the download, just log the error
        } else {
            println!("📝 Added regular download to history");
        }

        println!("Download started with ID: {}", actual_download_id);
        Ok(actual_download_id)
    }

    pub async fn pause_download(&self, download_id: &str) -> Result<()> {
        let download_type = {
            let downloads = self.downloads.read();
            downloads.get(download_id)
                .map(|d| d.download_type.clone())
                .ok_or_else(|| anyhow!("Download not found: {}", download_id))?
        };

        match download_type {
            DownloadType::Http => {
                self.aria2.pause_download(download_id).await?;
            }
            DownloadType::Torrent => {
                self.torrent.pause_torrent(download_id).await?;
            }
        }

        self.update_download_status(download_id, DownloadStatus::Paused).await;
        println!("Paused download: {}", download_id);
        Ok(())
    }

    pub async fn resume_download(&self, download_id: &str) -> Result<()> {
        let download_type = {
            let downloads = self.downloads.read();
            downloads.get(download_id)
                .map(|d| d.download_type.clone())
                .ok_or_else(|| anyhow!("Download not found: {}", download_id))?
        };

        match download_type {
            DownloadType::Http => {
                self.aria2.resume_download(download_id).await?;
            }
            DownloadType::Torrent => {
                self.torrent.resume_torrent(download_id).await?;
            }
        }

        self.update_download_status(download_id, DownloadStatus::Active).await;
        println!("Resumed download: {}", download_id);
        Ok(())
    }

    pub async fn cancel_download(&self, download_id: &str) -> Result<()> {
        let download_type = {
            let downloads = self.downloads.read();
            downloads.get(download_id)
                .map(|d| d.download_type.clone())
                .ok_or_else(|| anyhow!("Download not found: {}", download_id))?
        };

        match download_type {
            DownloadType::Http => {
                self.aria2.remove_download(download_id).await?;
            }
            DownloadType::Torrent => {
                self.torrent.remove_torrent(download_id).await?;
            }
        }

        // Update history as cancelled
        let _ = update_download_history_completion(
            download_id.to_string(),
            "cancelled".to_string(),
            0.0, // Progress at cancellation - we don't track this currently
            0,
            0,
            None,
            Some("Download cancelled by user".to_string()),
        ).await;

        {
            let mut downloads = self.downloads.write();
            downloads.remove(download_id);
        }

        println!("Cancelled download: {}", download_id);
        Ok(())
    }

    pub async fn get_download_progress(&self, download_id: &str) -> Result<DownloadProgress> {
        let download_type = {
            let downloads = self.downloads.read();
            downloads.get(download_id)
                .map(|d| d.download_type.clone())
                .ok_or_else(|| anyhow!("Download not found: {}", download_id))?
        };

        match download_type {
            DownloadType::Http => {
                self.aria2.get_download_status(download_id).await
            }
            DownloadType::Torrent => {
                self.torrent.get_torrent_status(download_id).await
            }
        }
    }

    pub async fn get_all_downloads(&self) -> Result<Vec<DownloadInfo>> {
        let downloads = self.downloads.read();
        Ok(downloads.values().cloned().collect())
    }

    pub async fn get_active_downloads(&self) -> Result<Vec<DownloadProgress>> {
        let mut all_progress = Vec::new();

        // Get aria2 downloads
        if let Ok(aria2_downloads) = self.aria2.get_all_downloads().await {
            all_progress.extend(aria2_downloads);
        }

        // Get torrent downloads
        if let Ok(torrent_downloads) = self.torrent.get_all_torrents().await {
            all_progress.extend(torrent_downloads);
        }

        Ok(all_progress)
    }

    async fn update_download_status(&self, download_id: &str, status: DownloadStatus) {
        let mut downloads = self.downloads.write();
        if let Some(download) = downloads.get_mut(download_id) {
            download.status = status.clone();
            download.progress.status = status;
            download.updated_at = Utc::now();
        }
    }

    async fn start_progress_monitoring(&self) {
        let aria2 = self.aria2.clone();
        let torrent = self.torrent.clone();
        let downloads = self.downloads.clone();
        let app_handle = self.app_handle.clone();
        let progress_sender = self.progress_sender.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3));
            
            loop {
                interval.tick().await;
                
                // Get all aria2 downloads but filter completed ones
                if let Ok(aria2_downloads) = aria2.get_all_downloads().await {
                    for progress in aria2_downloads {
                        // FILTER: Only process non-completed downloads
                        if progress.status != DownloadStatus::Completed && progress.progress < 1.0 {
                            // Send progress update to frontend
                            if let Some(ref app) = app_handle {
                                let _ = app.emit("download-progress", &progress);
                            }

                            // Send to progress channel if available
                            if let Some(ref sender) = progress_sender {
                                let _ = sender.send(progress.clone());
                            }

                            // Update download status in memory using actual aria2 ID
                            {
                                let mut downloads_lock = downloads.write();
                                // Find download by matching the actual aria2 download ID
                                for (_, download) in downloads_lock.iter_mut() {
                                    if download.progress.download_id == progress.download_id {
                                        download.progress = progress.clone();
                                        download.status = progress.status;
                                        download.updated_at = Utc::now();
                                        break;
                                    }
                                }
                            }
                        } else if progress.progress >= 1.0 && progress.status == DownloadStatus::Completed {
                            // Handle completed downloads - emit completion event
                            if let Some(ref app) = app_handle {
                                let _ = app.emit("download-complete", &progress);
                                // Also emit final progress update
                                let _ = app.emit("download-progress", &progress);
                            }

                            // Update history as completed
                            let _ = update_download_history_completion(
                                progress.download_id.clone(),
                                "completed".to_string(),
                                progress.progress,
                                progress.download_speed as i64,
                                0, // We don't track total time in progress monitoring
                                Some(progress.total_size as i64),
                                None,
                            ).await;
                            
                            // Delay removal to allow UI to process completion
                            let aria2_clone = aria2.clone();
                            let download_id_clone = progress.download_id.clone();
                            tokio::spawn(async move {
                                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                let _ = aria2_clone.remove_download(&download_id_clone).await;
                            });
                        }
                    }
                }
                
                // Get all torrent downloads but filter completed ones
                if let Ok(torrent_downloads) = torrent.get_all_torrents().await {
                    for progress in torrent_downloads {
                        // FILTER: Only process non-completed downloads
                        if progress.status != DownloadStatus::Completed && progress.progress < 1.0 {
                            // Send progress update to frontend
                            if let Some(ref app) = app_handle {
                                let _ = app.emit("download-progress", &progress);
                            }

                            // Send to progress channel if available
                            if let Some(ref sender) = progress_sender {
                                let _ = sender.send(progress.clone());
                            }

                            // Update download status in memory
                            {
                                let mut downloads_lock = downloads.write();
                                for (_, download) in downloads_lock.iter_mut() {
                                    if download.progress.download_id == progress.download_id {
                                        download.progress = progress.clone();
                                        download.status = progress.status;
                                        download.updated_at = Utc::now();
                                        break;
                                    }
                                }
                            }
                        } else if progress.progress >= 1.0 && progress.status == DownloadStatus::Completed {
                            // Handle completed downloads - emit completion event
                            if let Some(ref app) = app_handle {
                                let _ = app.emit("download-complete", &progress);
                                // Also emit final progress update
                                let _ = app.emit("download-progress", &progress);
                            }

                            // Update history as completed
                            let _ = update_download_history_completion(
                                progress.download_id.clone(),
                                "completed".to_string(),
                                progress.progress,
                                progress.download_speed as i64,
                                0, // We don't track total time in progress monitoring
                                Some(progress.total_size as i64),
                                None,
                            ).await;
                            
                            // Delay removal to allow UI to process completion
                            let torrent_clone = torrent.clone();
                            let download_id_clone = progress.download_id.clone();
                            tokio::spawn(async move {
                                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                let _ = torrent_clone.remove_torrent(&download_id_clone).await;
                            });
                        }
                    }
                }
            }
        });
    }

    async fn get_progress_for_download(
        aria2: &Arc<Aria2Service>,
        torrent: &Arc<TorrentDownloader>,
        downloads: &Arc<RwLock<HashMap<String, DownloadInfo>>>,
        download_id: &str,
    ) -> Result<DownloadProgress> {
        let download_type = {
            let downloads_lock = downloads.read();
            downloads_lock.get(download_id)
                .map(|d| d.download_type.clone())
                .ok_or_else(|| anyhow!("Download not found: {}", download_id))?
        };

        match download_type {
            DownloadType::Http => {
                aria2.get_download_status(download_id).await
            }
            DownloadType::Torrent => {
                torrent.get_torrent_status(download_id).await
            }
        }
    }

    // Utility method to determine download type from URL
    pub fn detect_download_type(url: &str) -> DownloadType {
        if url.starts_with("magnet:") || url.ends_with(".torrent") {
            DownloadType::Torrent
        } else {
            DownloadType::Http
        }
    }

    pub fn is_aria2_running(&self) -> bool {
        self.aria2.is_running()
    }
}
