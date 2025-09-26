use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RepoType {
    Branch,
    Encrypted,
    Decrypted,
    DirectZip,
    DirectUrl,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum UpdateStrategy {
    Smart,        // Pintar - otomatis cek semua source dan ambil terbaru
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateSource {
    pub name: String,
    pub url: String,
    pub repo_type: RepoType,
    pub priority: u8,
    pub is_reliable: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManifestInfo {
    pub depot_id: String,
    pub manifest_id: String,
    pub source: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateResult {
    pub success: bool,
    pub message: String,
    pub updated_count: u32,
    pub appended_count: u32,
    pub source_used: String,
    pub strategy_used: UpdateStrategy,
    pub has_newer_available: bool,
    pub newer_sources: Vec<String>,
}

