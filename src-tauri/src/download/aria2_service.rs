use crate::download::types::{Aria2Config, DownloadProgress, DownloadStatus};
use anyhow::{anyhow, Result};
use parking_lot::RwLock;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug)]
pub struct Aria2Service {
    config: Aria2Config,
    process: Arc<RwLock<Option<Child>>>,
    client: reqwest::Client,
    aria2_binary_path: PathBuf,
}

#[derive(Debug, serde::Deserialize)]
struct Aria2Response {
    id: String,
    jsonrpc: String,
    result: Option<Value>,
    error: Option<Aria2Error>,
}

#[derive(Debug, serde::Deserialize)]
struct Aria2Error {
    code: i32,
    message: String,
}

impl Aria2Service {
    pub fn new(aria2_binary_path: PathBuf) -> Result<Self> {
        let config = Aria2Config::default();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            config,
            process: Arc::new(RwLock::new(None)),
            client,
            aria2_binary_path,
        })
    }

    pub async fn start(&self) -> Result<()> {
        // Check if aria2c binary exists
        if !self.aria2_binary_path.exists() {
            return Err(anyhow!("aria2c binary not found at {:?}", self.aria2_binary_path));
        }

        // Stop existing process if running
        self.stop().await?;

        let mut cmd = Command::new(&self.aria2_binary_path);
        cmd.args([
            "--enable-rpc",
            "--rpc-listen-all=false", // Only listen on localhost for security
            "--rpc-listen-port", &self.config.port.to_string(),
            "--file-allocation=none",
            "--allow-overwrite=true",
            "--auto-file-renaming=false",
            "--continue=true",
            "--max-concurrent-downloads", &self.config.max_concurrent_downloads.to_string(),
            "--max-connection-per-server", &self.config.max_connections_per_server.to_string(),
            "--split", &self.config.split.to_string(),
            "--min-split-size", &self.config.min_split_size,
            "--disable-ipv6=true",
            "--summary-interval=1",
        ]);

        if let Some(secret) = &self.config.secret {
            cmd.args(["--rpc-secret", secret]);
        }

        let child = cmd
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        {
            let mut process = self.process.write();
            *process = Some(child);
        }

        // Wait for aria2c to start
        self.wait_for_ready().await?;

        println!("aria2c started successfully on port {}", self.config.port);
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let mut process = self.process.write();
        if let Some(mut child) = process.take() {
            let _ = child.kill();
            let _ = child.wait();
            println!("aria2c process stopped");
        }
        Ok(())
    }

    async fn wait_for_ready(&self) -> Result<()> {
        for _ in 0..30 { // Wait up to 30 seconds
            if self.is_ready().await {
                return Ok(());
            }
            sleep(Duration::from_secs(1)).await;
        }
        Err(anyhow!("aria2c failed to start within 30 seconds"))
    }

    async fn is_ready(&self) -> bool {
        self.rpc_call("aria2.getVersion", &[]).await.is_ok()
    }

    async fn rpc_call(&self, method: &str, params: &[Value]) -> Result<Value> {
        let url = format!("http://{}:{}/jsonrpc", self.config.host, self.config.port);
        
        let mut request_params = vec![];
        if let Some(secret) = &self.config.secret {
            request_params.push(json!(format!("token:{}", secret)));
        }
        request_params.extend_from_slice(params);

        let request_body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": request_params,
            "id": "1"
        });

        let response = self.client
            .post(&url)
            .json(&request_body)
            .send()
            .await?;

        let aria2_response: Aria2Response = response.json().await?;

        if let Some(error) = aria2_response.error {
            return Err(anyhow!("aria2 RPC error: {} (code: {})", error.message, error.code));
        }

        aria2_response.result.ok_or_else(|| anyhow!("No result in aria2 response"))
    }

    pub async fn add_download(
        &self,
        url: &str,
        save_path: &str,
        filename: Option<&str>,
        headers: Option<&HashMap<String, String>>,
    ) -> Result<String> {
        let mut options = json!({
            "dir": save_path,
            "continue": "true",
            "allow-overwrite": "true"
        });

        if let Some(name) = filename {
            options["out"] = json!(name);
        }

        if let Some(headers_map) = headers {
            let header_string = headers_map
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join("\n");
            options["header"] = json!(header_string);
        }

        let result = self.rpc_call("aria2.addUri", &[
            json!([url]),
            options
        ]).await?;

        result.as_str()
            .ok_or_else(|| anyhow!("Invalid response format for addUri"))
            .map(|s| s.to_string())
    }

    pub async fn get_download_status(&self, gid: &str) -> Result<DownloadProgress> {
        let result = self.rpc_call("aria2.tellStatus", &[json!(gid)]).await?;
        self.parse_download_status(result)
    }

    pub async fn pause_download(&self, gid: &str) -> Result<()> {
        self.rpc_call("aria2.pause", &[json!(gid)]).await?;
        Ok(())
    }

    pub async fn resume_download(&self, gid: &str) -> Result<()> {
        self.rpc_call("aria2.unpause", &[json!(gid)]).await?;
        Ok(())
    }

    pub async fn remove_download(&self, gid: &str) -> Result<()> {
        // Try to remove from active downloads first
        let _ = self.rpc_call("aria2.remove", &[json!(gid)]).await;
        // Then remove from stopped downloads
        let _ = self.rpc_call("aria2.removeDownloadResult", &[json!(gid)]).await;
        Ok(())
    }

    pub async fn get_all_downloads(&self) -> Result<Vec<DownloadProgress>> {
        let active = self.rpc_call("aria2.tellActive", &[]).await.unwrap_or(json!([]));
        let waiting = self.rpc_call("aria2.tellWaiting", &[json!(0), json!(1000)]).await.unwrap_or(json!([]));
        let stopped = self.rpc_call("aria2.tellStopped", &[json!(0), json!(1000)]).await.unwrap_or(json!([]));

        let mut downloads = Vec::new();

        for status_list in [&active, &waiting, &stopped] {
            if let Some(array) = status_list.as_array() {
                for status in array {
                    if let Ok(progress) = self.parse_download_status(status.clone()) {
                        downloads.push(progress);
                    }
                }
            }
        }

        Ok(downloads)
    }

    fn parse_download_status(&self, status: Value) -> Result<DownloadProgress> {
        let gid = status["gid"].as_str().ok_or_else(|| anyhow!("Missing gid"))?;
        let status_str = status["status"].as_str().ok_or_else(|| anyhow!("Missing status"))?;
        
        let download_status = match status_str {
            "active" => DownloadStatus::Active,
            "waiting" => DownloadStatus::Pending,
            "paused" => DownloadStatus::Paused,
            "complete" => DownloadStatus::Completed,
            "error" => DownloadStatus::Error,
            "removed" => DownloadStatus::Cancelled,
            _ => DownloadStatus::Pending,
        };

        let total_length = status["totalLength"].as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        
        let completed_length = status["completedLength"].as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let download_speed = status["downloadSpeed"].as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let upload_speed = status["uploadSpeed"].as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let progress = if total_length > 0 {
            completed_length as f64 / total_length as f64
        } else {
            0.0
        };

        let eta = if download_speed > 0 && total_length > completed_length {
            Some((total_length - completed_length) / download_speed)
        } else {
            None
        };

        let file_name = status["files"].as_array()
            .and_then(|files| files.first())
            .and_then(|file| file["path"].as_str())
            .and_then(|path| std::path::Path::new(path).file_name())
            .and_then(|name| name.to_str())
            .map(|s| s.to_string());

        Ok(DownloadProgress {
            download_id: gid.to_string(),
            progress,
            download_speed,
            upload_speed,
            total_size: total_length,
            downloaded_size: completed_length,
            eta,
            num_peers: 0,
            num_seeds: 0,
            status: download_status,
            file_name,
        })
    }

    pub fn is_running(&self) -> bool {
        let process = self.process.read();
        process.is_some()
    }
}

impl Drop for Aria2Service {
    fn drop(&mut self) {
        let _ = futures::executor::block_on(self.stop());
    }
}
