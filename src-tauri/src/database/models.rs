use chrono::Utc;
use rusqlite::{Row, Result as SqliteResult};
use serde::{Deserialize, Serialize};

/// Database model for games table (basic game info)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub app_id: String,
    pub name: String,
    pub header_image: String,
    pub cached_at: i64,
    pub expires_at: i64,
    pub last_updated: i64,
}

impl Game {
    /// Create a new Game with TTL
    pub fn new(app_id: String, name: String, header_image: String, ttl_seconds: i64) -> Self {
        let now = Utc::now().timestamp();
        Self {
            app_id,
            name,
            header_image,
            cached_at: now,
            expires_at: now + ttl_seconds,
            last_updated: now,
        }
    }

    /// Check if this game entry is expired
    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp() > self.expires_at
    }

    /// Convert from SQLite row
    pub fn from_row(row: &Row) -> SqliteResult<Self> {
        Ok(Self {
            app_id: row.get(0)?,
            name: row.get(1)?,
            header_image: row.get(2)?,
            cached_at: row.get(3)?,
            expires_at: row.get(4)?,
            last_updated: row.get(5)?,
        })
    }
}

/// Database model for game_details table (detailed game information) with granular TTL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDetailDb {
    pub app_id: String,
    pub name: String,
    pub header_image: String,
    pub banner_image: String,
    pub detailed_description: String,
    pub release_date: String,
    pub publisher: String,
    pub trailer: Option<String>,
    pub screenshots: Vec<String>,
    pub sysreq_min: Vec<(String, String)>,
    pub sysreq_rec: Vec<(String, String)>,
    pub pc_requirements: Option<PcRequirements>,
    pub dlc: Vec<String>,
    pub drm_notice: Option<String>,
    
    // Global cache timestamps (for backward compatibility)
    pub cached_at: i64,
    pub expires_at: i64,
    pub last_updated: i64,
    
    // Granular expiry timestamps for different data categories
    pub dynamic_expires_at: i64,    // For DLC list (3 days)
    pub semistatic_expires_at: i64, // For name, images, trailer (3 weeks)
    pub static_expires_at: i64,     // For screenshots, descriptions, sysreq (60+ days)
}

impl GameDetailDb {
    /// Create a new GameDetailDb with granular TTL
    pub fn new(
        app_id: String,
        name: String,
        header_image: String,
        banner_image: String,
        detailed_description: String,
        release_date: String,
        publisher: String,
        trailer: Option<String>,
        screenshots: Vec<String>,
        sysreq_min: Vec<(String, String)>,
        sysreq_rec: Vec<(String, String)>,
        pc_requirements: Option<PcRequirements>,
        dlc: Vec<String>,
        drm_notice: Option<String>,
    ) -> Self {
        let now = Utc::now().timestamp();
        
        // Import TTL config
        use crate::database::ttl_config::TtlConfig;
        
        Self {
            app_id,
            name,
            header_image,
            banner_image,
            detailed_description,
            release_date,
            publisher,
            trailer,
            screenshots,
            sysreq_min,
            sysreq_rec,
            pc_requirements,
            dlc,
            drm_notice,
            cached_at: now,
            expires_at: now + TtlConfig::DEFAULT, // Use default for global expiry
            last_updated: now,
            // Granular expiry based on data category
            dynamic_expires_at: now + TtlConfig::DLC_LIST,
            semistatic_expires_at: now + TtlConfig::GAME_NAME, // Use shortest semi-static TTL
            static_expires_at: now + TtlConfig::SCREENSHOTS, // Use shortest static TTL
        }
    }

    /// Create GameDetailDb with custom TTL (for backward compatibility)
    pub fn with_ttl(
        app_id: String,
        name: String,
        header_image: String,
        banner_image: String,
        detailed_description: String,
        release_date: String,
        publisher: String,
        trailer: Option<String>,
        screenshots: Vec<String>,
        sysreq_min: Vec<(String, String)>,
        sysreq_rec: Vec<(String, String)>,
        pc_requirements: Option<PcRequirements>,
        dlc: Vec<String>,
        drm_notice: Option<String>,
        ttl_seconds: i64,
    ) -> Self {
        let now = Utc::now().timestamp();
        
        Self {
            app_id,
            name,
            header_image,
            banner_image,
            detailed_description,
            release_date,
            publisher,
            trailer,
            screenshots,
            sysreq_min,
            sysreq_rec,
            pc_requirements,
            dlc,
            drm_notice,
            cached_at: now,
            expires_at: now + ttl_seconds,
            last_updated: now,
            // Use same TTL for all categories when using legacy method
            dynamic_expires_at: now + ttl_seconds,
            semistatic_expires_at: now + ttl_seconds,
            static_expires_at: now + ttl_seconds,
        }
    }

    /// Check if this game detail entry is expired (conservative check)
    /// Only considers data expired if the dynamic data (most important) is expired
    /// This prevents unnecessary API calls for stable data like screenshots
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp();
        
        // Only force refresh if dynamic data (DLC) is expired
        // For other expired categories, use stale-while-revalidate pattern
        now > self.dynamic_expires_at
    }
    
    /// Check if this game detail entry is expired (legacy global check)
    /// Kept for backward compatibility
    pub fn is_expired_global(&self) -> bool {
        Utc::now().timestamp() > self.expires_at
    }

    /// Check if dynamic data (DLC list) is expired
    pub fn is_dynamic_expired(&self) -> bool {
        Utc::now().timestamp() > self.dynamic_expires_at
    }

    /// Check if semi-static data (name, images, trailer) is expired
    pub fn is_semistatic_expired(&self) -> bool {
        Utc::now().timestamp() > self.semistatic_expires_at
    }

    /// Check if static data (screenshots, descriptions, sysreq) is expired
    pub fn is_static_expired(&self) -> bool {
        Utc::now().timestamp() > self.static_expires_at
    }

    /// Get list of expired data categories
    pub fn get_expired_categories(&self) -> Vec<&'static str> {
        let mut expired = Vec::new();
        let now = Utc::now().timestamp();

        if now > self.dynamic_expires_at {
            expired.push("dynamic");
        }
        if now > self.semistatic_expires_at {
            expired.push("semistatic");
        }
        if now > self.static_expires_at {
            expired.push("static");
        }

        expired
    }

    /// Check if any category is expired
    pub fn has_any_expired(&self) -> bool {
        let now = Utc::now().timestamp();
        now > self.dynamic_expires_at || 
        now > self.semistatic_expires_at || 
        now > self.static_expires_at
    }

    /// Update specific category expiry
    pub fn refresh_category_expiry(&mut self, category: &str) {
        let now = Utc::now().timestamp();
        use crate::database::ttl_config::TtlConfig;

        match category {
            "dynamic" => {
                self.dynamic_expires_at = now + TtlConfig::DLC_LIST;
            }
            "semistatic" => {
                self.semistatic_expires_at = now + TtlConfig::GAME_NAME;
            }
            "static" => {
                self.static_expires_at = now + TtlConfig::SCREENSHOTS;
            }
            _ => {
                // Refresh all categories
                self.dynamic_expires_at = now + TtlConfig::DLC_LIST;
                self.semistatic_expires_at = now + TtlConfig::GAME_NAME;
                self.static_expires_at = now + TtlConfig::SCREENSHOTS;
            }
        }
        
        self.last_updated = now;
    }

    /// Convert from SQLite row
    pub fn from_row(row: &Row) -> SqliteResult<Self> {
        let screenshots_json: String = row.get(8)?;
        let sysreq_min_json: String = row.get(9)?;
        let sysreq_rec_json: String = row.get(10)?;
        let pc_requirements_json: Option<String> = row.get(11)?;
        let dlc_json: String = row.get(12)?;

        Ok(Self {
            app_id: row.get(0)?,
            name: row.get(1)?,
            header_image: row.get(2)?,
            banner_image: row.get(3)?,
            detailed_description: row.get(4)?,
            release_date: row.get(5)?,
            publisher: row.get(6)?,
            trailer: row.get(7)?,
            screenshots: serde_json::from_str(&screenshots_json).unwrap_or_default(),
            sysreq_min: serde_json::from_str(&sysreq_min_json).unwrap_or_default(),
            sysreq_rec: serde_json::from_str(&sysreq_rec_json).unwrap_or_default(),
            pc_requirements: pc_requirements_json
                .and_then(|json| serde_json::from_str(&json).ok()),
            dlc: serde_json::from_str(&dlc_json).unwrap_or_default(),
            drm_notice: row.get(13)?,
            cached_at: row.get(14)?,
            expires_at: row.get(15)?,
            last_updated: row.get(16)?,
            // Granular TTL fields
            dynamic_expires_at: row.get(17)?,
            semistatic_expires_at: row.get(18)?,
            static_expires_at: row.get(19)?,
        })
    }
}

/// PC Requirements structure (matches existing code)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcRequirements {
    pub minimum: Option<String>,
    pub recommended: Option<String>,
}

/// Database model for user_library table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLibraryEntry {
    pub app_id: String,
    pub added_at: i64,
    pub last_accessed: Option<i64>,
    pub access_count: i64,
}

impl UserLibraryEntry {
    /// Create a new library entry
    pub fn new(app_id: String) -> Self {
        Self {
            app_id,
            added_at: Utc::now().timestamp(),
            last_accessed: None,
            access_count: 0,
        }
    }

    /// Update access tracking
    pub fn update_access(&mut self) {
        self.last_accessed = Some(Utc::now().timestamp());
        self.access_count += 1;
    }

    /// Convert from SQLite row
    pub fn from_row(row: &Row) -> SqliteResult<Self> {
        Ok(Self {
            app_id: row.get(0)?,
            added_at: row.get(1)?,
            last_accessed: row.get(2)?,
            access_count: row.get(3)?,
        })
    }
}

/// Cache metadata entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    pub key: String,
    pub value: String,
    pub updated_at: i64,
}

impl CacheMetadata {
    /// Create new metadata entry
    pub fn new(key: String, value: String) -> Self {
        Self {
            key,
            value,
            updated_at: Utc::now().timestamp(),
        }
    }

    /// Convert from SQLite row
    pub fn from_row(row: &Row) -> SqliteResult<Self> {
        Ok(Self {
            key: row.get(0)?,
            value: row.get(1)?,
            updated_at: row.get(2)?,
        })
    }
}

/// User profile data with TTL support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: i32,
    pub name: String,
    pub steam_id: Option<String>,
    pub banner_path: Option<String>,
    pub avatar_path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub cached_at: i64,
    pub expires_at: i64,
    pub is_backed_up: bool,
    pub backup_created_at: i64,
}

impl UserProfile {
    /// Create new user profile with TTL
    pub fn new(name: String) -> Self {
        let now = Utc::now().timestamp();
        Self {
            id: 1, // Single profile entry
            name,
            steam_id: None,
            banner_path: None,
            avatar_path: None,
            created_at: now,
            updated_at: now,
            cached_at: now,
            expires_at: now + 31536000, // 1 year (365 * 24 * 3600)
            is_backed_up: false,
            backup_created_at: 0,
        }
    }

    /// Check if profile is expired
    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp() > self.expires_at
    }

    /// Refresh TTL
    pub fn refresh_ttl(&mut self) {
        let now = Utc::now().timestamp();
        self.cached_at = now;
        self.expires_at = now + 31536000; // 1 year (365 * 24 * 3600)
        self.updated_at = now;
    }

    /// Mark as backed up
    pub fn mark_backed_up(&mut self) {
        self.is_backed_up = true;
        self.backup_created_at = Utc::now().timestamp();
    }

    /// Convert from SQLite row
    pub fn from_row(row: &Row) -> SqliteResult<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            steam_id: row.get(2)?,
            banner_path: row.get(3)?,
            avatar_path: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
            cached_at: row.get(7).unwrap_or_else(|_| Utc::now().timestamp()),
            expires_at: row.get(8).unwrap_or_else(|_| Utc::now().timestamp() + 31536000), // 1 year
            is_backed_up: row.get::<_, i32>(9).unwrap_or(0) == 1,
            backup_created_at: row.get(10).unwrap_or(0),
        })
    }

    /// Update timestamp
    pub fn touch(&mut self) {
        self.updated_at = Utc::now().timestamp();
    }
}

// Conversion functions to/from existing structs

/// Convert existing GameDetail struct to GameDetailDb
impl From<crate::GameDetail> for GameDetailDb {
    fn from(detail: crate::GameDetail) -> Self {
        Self::new(
            detail.app_id,
            detail.name,
            detail.header_image,
            detail.banner_image,
            detail.detailed_description,
            detail.release_date,
            detail.publisher,
            detail.trailer,
            detail.screenshots,
            detail.sysreq_min,
            detail.sysreq_rec,
            detail.pc_requirements.map(|req| PcRequirements {
                minimum: req.minimum,
                recommended: req.recommended,
            }),
            detail.dlc,
            detail.drm_notice,
        )
    }
}

/// Convert GameDetailDb to existing GameDetail struct
impl From<GameDetailDb> for crate::GameDetail {
    fn from(detail: GameDetailDb) -> Self {
        Self {
            app_id: detail.app_id,
            name: detail.name,
            header_image: detail.header_image,
            banner_image: detail.banner_image,
            detailed_description: detail.detailed_description,
            release_date: detail.release_date,
            publisher: detail.publisher,
            trailer: detail.trailer,
            screenshots: detail.screenshots,
            sysreq_min: detail.sysreq_min,
            sysreq_rec: detail.sysreq_rec,
            pc_requirements: detail.pc_requirements.map(|req| crate::PcRequirements {
                minimum: req.minimum,
                recommended: req.recommended,
            }),
            dlc: detail.dlc,
            drm_notice: detail.drm_notice,
        }
    }
}

/// Convert existing LibraryGame struct to Game
impl From<crate::LibraryGame> for Game {
    fn from(game: crate::LibraryGame) -> Self {
        Self::new(
            game.app_id,
            game.name,
            game.header_image,
            604800, // 7 days TTL for library games
        )
    }
}

/// Convert Game to existing LibraryGame struct
impl From<Game> for crate::LibraryGame {
    fn from(game: Game) -> Self {
        Self {
            app_id: game.app_id,
            name: game.name,
            header_image: game.header_image,
        }
    }
}

/// Database model for bypass_games table (static data with monthly TTL)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BypassGame {
    pub app_id: String,
    pub name: String,
    pub image: String,
    pub bypasses: Vec<BypassInfo>,
    pub cached_at: i64,
    pub expires_at: i64,
    pub last_updated: i64,
}

/// Bypass info structure (matches frontend JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BypassInfo {
    pub r#type: u8,
    pub url: String,
}

impl BypassGame {
    /// Create a new BypassGame with 1 month TTL (static data)
    pub fn new(app_id: String, name: String, image: String, bypasses: Vec<BypassInfo>) -> Self {
        let now = chrono::Utc::now().timestamp();
        let one_month = 30 * 24 * 60 * 60; // 1 month in seconds
        
        Self {
            app_id,
            name,
            image,
            bypasses,
            cached_at: now,
            expires_at: now + one_month,
            last_updated: now,
        }
    }

    /// Check if this bypass game entry is expired
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().timestamp() > self.expires_at
    }

    /// Convert from SQLite row
    pub fn from_row(row: &Row) -> SqliteResult<Self> {
        let bypasses_json: String = row.get(3)?;
        
        Ok(Self {
            app_id: row.get(0)?,
            name: row.get(1)?,
            image: row.get(2)?,
            bypasses: serde_json::from_str(&bypasses_json).unwrap_or_default(),
            cached_at: row.get(4)?,
            expires_at: row.get(5)?,
            last_updated: row.get(6)?,
        })
    }
}
