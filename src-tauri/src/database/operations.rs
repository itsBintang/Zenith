use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use crate::database::models::{Game, GameDetailDb, UserLibraryEntry, CacheMetadata, UserProfile, BypassGame};

/// Game operations
pub struct GameOperations;

impl GameOperations {
    /// Insert or update a game
    pub fn upsert(conn: &Connection, game: &Game) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO games 
             (app_id, name, header_image, cached_at, expires_at, last_updated) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                game.app_id,
                game.name,
                game.header_image,
                game.cached_at,
                game.expires_at,
                game.last_updated
            ],
        )?;
        Ok(())
    }

    /// Get a game by app_id
    pub fn get_by_id(conn: &Connection, app_id: &str) -> Result<Option<Game>> {
        let mut stmt = conn.prepare(
            "SELECT app_id, name, header_image, cached_at, expires_at, last_updated 
             FROM games WHERE app_id = ?1"
        )?;
        
        let game = stmt.query_row([app_id], |row| Game::from_row(row))
            .optional()?;
        
        Ok(game)
    }

    /// Get all games (with optional limit)
    pub fn get_all(conn: &Connection, limit: Option<u32>) -> Result<Vec<Game>> {
        let mut games = Vec::new();
        
        if let Some(l) = limit {
            let mut stmt = conn.prepare(
                "SELECT app_id, name, header_image, cached_at, expires_at, last_updated 
                 FROM games ORDER BY name LIMIT ?1"
            )?;
            let game_iter = stmt.query_map([l], |row| Game::from_row(row))?;
            for game_result in game_iter {
                games.push(game_result?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT app_id, name, header_image, cached_at, expires_at, last_updated 
                 FROM games ORDER BY name"
            )?;
            let game_iter = stmt.query_map([], |row| Game::from_row(row))?;
            for game_result in game_iter {
                games.push(game_result?);
            }
        }
        
        Ok(games)
    }

    /// Search games by name
    pub fn search_by_name(conn: &Connection, query: &str, limit: Option<u32>) -> Result<Vec<Game>> {
        let search_pattern = format!("%{}%", query.to_lowercase());
        let mut games = Vec::new();
        
        if let Some(l) = limit {
            let mut stmt = conn.prepare(
                "SELECT app_id, name, header_image, cached_at, expires_at, last_updated 
                 FROM games WHERE LOWER(name) LIKE ?1 ORDER BY name LIMIT ?2"
            )?;
            let game_iter = stmt.query_map(params![search_pattern, l], |row| Game::from_row(row))?;
            for game_result in game_iter {
                games.push(game_result?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT app_id, name, header_image, cached_at, expires_at, last_updated 
                 FROM games WHERE LOWER(name) LIKE ?1 ORDER BY name"
            )?;
            let game_iter = stmt.query_map([search_pattern], |row| Game::from_row(row))?;
            for game_result in game_iter {
                games.push(game_result?);
            }
        }
        
        Ok(games)
    }

    /// Delete a game
    pub fn delete(conn: &Connection, app_id: &str) -> Result<bool> {
        let rows_affected = conn.execute("DELETE FROM games WHERE app_id = ?1", [app_id])?;
        Ok(rows_affected > 0)
    }

    /// Get expired games
    pub fn get_expired(conn: &Connection) -> Result<Vec<Game>> {
        let now = chrono::Utc::now().timestamp();
        let mut stmt = conn.prepare(
            "SELECT app_id, name, header_image, cached_at, expires_at, last_updated 
             FROM games WHERE expires_at < ?1"
        )?;
        
        let game_iter = stmt.query_map([now], |row| Game::from_row(row))?;
        let mut games = Vec::new();
        for game_result in game_iter {
            games.push(game_result?);
        }
        
        Ok(games)
    }
}

/// Game details operations
pub struct GameDetailOperations;

impl GameDetailOperations {
    /// Insert or update game details
    pub fn upsert(conn: &Connection, detail: &GameDetailDb) -> Result<()> {
        let screenshots_json = serde_json::to_string(&detail.screenshots)?;
        let sysreq_min_json = serde_json::to_string(&detail.sysreq_min)?;
        let sysreq_rec_json = serde_json::to_string(&detail.sysreq_rec)?;
        let pc_requirements_json = detail.pc_requirements.as_ref()
            .map(|req| serde_json::to_string(req))
            .transpose()?;
        let dlc_json = serde_json::to_string(&detail.dlc)?;
        
        conn.execute(
            "INSERT OR REPLACE INTO game_details 
             (app_id, name, header_image, banner_image, detailed_description, 
              release_date, publisher, trailer, screenshots, sysreq_min, sysreq_rec, 
              pc_requirements, dlc, drm_notice, cached_at, expires_at, last_updated,
              dynamic_expires_at, semistatic_expires_at, static_expires_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                detail.app_id,
                detail.name,
                detail.header_image,
                detail.banner_image,
                detail.detailed_description,
                detail.release_date,
                detail.publisher,
                detail.trailer,
                screenshots_json,
                sysreq_min_json,
                sysreq_rec_json,
                pc_requirements_json,
                dlc_json,
                detail.drm_notice,
                detail.cached_at,
                detail.expires_at,
                detail.last_updated,
                detail.dynamic_expires_at,
                detail.semistatic_expires_at,
                detail.static_expires_at
            ],
        )?;
        Ok(())
    }

    /// Get game details by app_id
    pub fn get_by_id(conn: &Connection, app_id: &str) -> Result<Option<GameDetailDb>> {
        let mut stmt = conn.prepare(
            "SELECT app_id, name, header_image, banner_image, detailed_description, 
                    release_date, publisher, trailer, screenshots, sysreq_min, sysreq_rec, 
                    pc_requirements, dlc, drm_notice, cached_at, expires_at, last_updated,
                    dynamic_expires_at, semistatic_expires_at, static_expires_at 
             FROM game_details WHERE app_id = ?1"
        )?;
        
        let detail = stmt.query_row([app_id], |row| GameDetailDb::from_row(row))
            .optional()?;
        
        Ok(detail)
    }

    /// Delete game details
    pub fn delete(conn: &Connection, app_id: &str) -> Result<bool> {
        let rows_affected = conn.execute("DELETE FROM game_details WHERE app_id = ?1", [app_id])?;
        Ok(rows_affected > 0)
    }

    /// Get expired game details (global expiry)
    pub fn get_expired(conn: &Connection) -> Result<Vec<GameDetailDb>> {
        let now = chrono::Utc::now().timestamp();
        let mut stmt = conn.prepare(
            "SELECT app_id, name, header_image, banner_image, detailed_description, 
                    release_date, publisher, trailer, screenshots, sysreq_min, sysreq_rec, 
                    pc_requirements, dlc, drm_notice, cached_at, expires_at, last_updated,
                    dynamic_expires_at, semistatic_expires_at, static_expires_at 
             FROM game_details WHERE expires_at < ?1"
        )?;
        
        let detail_iter = stmt.query_map([now], |row| GameDetailDb::from_row(row))?;
        let mut details = Vec::new();
        for detail_result in detail_iter {
            details.push(detail_result?);
        }
        
        Ok(details)
    }

    /// Get games with expired dynamic data (DLC list)
    pub fn get_dynamic_expired(conn: &Connection) -> Result<Vec<GameDetailDb>> {
        let now = chrono::Utc::now().timestamp();
        let mut stmt = conn.prepare(
            "SELECT app_id, name, header_image, banner_image, detailed_description, 
                    release_date, publisher, trailer, screenshots, sysreq_min, sysreq_rec, 
                    pc_requirements, dlc, drm_notice, cached_at, expires_at, last_updated,
                    dynamic_expires_at, semistatic_expires_at, static_expires_at 
             FROM game_details WHERE dynamic_expires_at < ?1"
        )?;
        
        let detail_iter = stmt.query_map([now], |row| GameDetailDb::from_row(row))?;
        let mut details = Vec::new();
        for detail_result in detail_iter {
            details.push(detail_result?);
        }
        
        Ok(details)
    }

    /// Get games with any expired category
    pub fn get_any_expired(conn: &Connection) -> Result<Vec<GameDetailDb>> {
        let now = chrono::Utc::now().timestamp();
        let mut stmt = conn.prepare(
            "SELECT app_id, name, header_image, banner_image, detailed_description, 
                    release_date, publisher, trailer, screenshots, sysreq_min, sysreq_rec, 
                    pc_requirements, dlc, drm_notice, cached_at, expires_at, last_updated,
                    dynamic_expires_at, semistatic_expires_at, static_expires_at 
             FROM game_details WHERE dynamic_expires_at < ?1 OR semistatic_expires_at < ?1 OR static_expires_at < ?1"
        )?;
        
        let detail_iter = stmt.query_map([now], |row| GameDetailDb::from_row(row))?;
        let mut details = Vec::new();
        for detail_result in detail_iter {
            details.push(detail_result?);
        }
        
        Ok(details)
    }
}

/// User library operations
pub struct UserLibraryOperations;

impl UserLibraryOperations {
    /// Add game to user library
    pub fn add_game(conn: &Connection, app_id: &str) -> Result<()> {
        let entry = UserLibraryEntry::new(app_id.to_string());
        conn.execute(
            "INSERT OR IGNORE INTO user_library (app_id, added_at, last_accessed, access_count) 
             VALUES (?1, ?2, ?3, ?4)",
            params![entry.app_id, entry.added_at, entry.last_accessed, entry.access_count],
        )?;
        Ok(())
    }

    /// Remove game from user library
    pub fn remove_game(conn: &Connection, app_id: &str) -> Result<bool> {
        let rows_affected = conn.execute("DELETE FROM user_library WHERE app_id = ?1", [app_id])?;
        Ok(rows_affected > 0)
    }

    /// Update game access
    pub fn update_access(conn: &Connection, app_id: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE user_library 
             SET last_accessed = ?1, access_count = access_count + 1 
             WHERE app_id = ?2",
            params![now, app_id],
        )?;
        Ok(())
    }

    /// Get user library games
    pub fn get_library_games(conn: &Connection) -> Result<Vec<UserLibraryEntry>> {
        let mut stmt = conn.prepare(
            "SELECT app_id, added_at, last_accessed, access_count 
             FROM user_library ORDER BY added_at DESC"
        )?;
        
        let entry_iter = stmt.query_map([], |row| UserLibraryEntry::from_row(row))?;
        let mut entries = Vec::new();
        for entry_result in entry_iter {
            entries.push(entry_result?);
        }
        
        Ok(entries)
    }

    /// Check if game is in library
    pub fn is_in_library(conn: &Connection, app_id: &str) -> Result<bool> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM user_library WHERE app_id = ?1",
            [app_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

/// Cache metadata operations
pub struct CacheMetadataOperations;

impl CacheMetadataOperations {
    /// Set metadata value
    pub fn set(conn: &Connection, key: &str, value: &str) -> Result<()> {
        let metadata = CacheMetadata::new(key.to_string(), value.to_string());
        conn.execute(
            "INSERT OR REPLACE INTO cache_metadata (key, value, updated_at) 
             VALUES (?1, ?2, ?3)",
            params![metadata.key, metadata.value, metadata.updated_at],
        )?;
        Ok(())
    }

    /// Get metadata value
    pub fn get(conn: &Connection, key: &str) -> Result<Option<String>> {
        let value = conn.query_row(
            "SELECT value FROM cache_metadata WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        ).optional()?;
        Ok(value)
    }

    /// Get all metadata
    pub fn get_all(conn: &Connection) -> Result<Vec<CacheMetadata>> {
        let mut stmt = conn.prepare(
            "SELECT key, value, updated_at FROM cache_metadata ORDER BY key"
        )?;
        
        let metadata_iter = stmt.query_map([], |row| CacheMetadata::from_row(row))?;
        let mut metadata = Vec::new();
        for metadata_result in metadata_iter {
            metadata.push(metadata_result?);
        }
        
        Ok(metadata)
    }

    /// Delete metadata
    pub fn delete(conn: &Connection, key: &str) -> Result<bool> {
        let rows_affected = conn.execute("DELETE FROM cache_metadata WHERE key = ?1", [key])?;
        Ok(rows_affected > 0)
    }
}

/// User profile operations
pub struct UserProfileOperations;

impl UserProfileOperations {
    /// Get user profile (always returns the single profile entry)
    pub fn get(conn: &Connection) -> Result<Option<UserProfile>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, bio, steam_id, banner_path, avatar_path, created_at, updated_at,
                    cached_at, expires_at, is_backed_up, backup_created_at 
             FROM user_profile WHERE id = 1"
        )?;
        
        let profile = stmt.query_row([], |row| UserProfile::from_row(row))
            .optional()?;
        
        Ok(profile)
    }

    /// Create automatic backup before updating profile
    pub fn create_backup(conn: &Connection, reason: &str) -> Result<()> {
        // Get current profile
        if let Some(profile) = Self::get(conn)? {
            conn.execute(
                "INSERT OR REPLACE INTO user_profile_backup 
                 (id, name, bio, steam_id, banner_path, avatar_path, created_at, updated_at,
                  cached_at, expires_at, backup_created_at, backup_reason) 
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    profile.id,
                    profile.name,
                    profile.bio,
                    profile.steam_id,
                    profile.banner_path,
                    profile.avatar_path,
                    profile.created_at,
                    profile.updated_at,
                    profile.cached_at,
                    profile.expires_at,
                    chrono::Utc::now().timestamp(),
                    reason
                ],
            )?;
        }
        Ok(())
    }

    /// Restore profile from backup
    pub fn restore_from_backup(conn: &Connection) -> Result<bool> {
        let backup_exists: bool = conn.prepare(
            "SELECT 1 FROM user_profile_backup WHERE id = 1"
        )?.exists([])?;

        if backup_exists {
            conn.execute(
                "INSERT OR REPLACE INTO user_profile 
                 (id, name, bio, steam_id, banner_path, avatar_path, created_at, updated_at,
                  cached_at, expires_at, is_backed_up, backup_created_at)
                 SELECT id, name, bio, steam_id, banner_path, avatar_path, created_at, updated_at,
                        cached_at, expires_at, 1, backup_created_at
                 FROM user_profile_backup WHERE id = 1",
                [],
            )?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Insert or update user profile with automatic backup
    pub fn upsert(conn: &Connection, profile: &UserProfile) -> Result<()> {
        // Create backup before updating
        let _ = Self::create_backup(conn, "auto_update");
        
        conn.execute(
            "INSERT OR REPLACE INTO user_profile 
             (id, name, bio, steam_id, banner_path, avatar_path, created_at, updated_at,
              cached_at, expires_at, is_backed_up, backup_created_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                profile.id,
                profile.name,
                profile.bio,
                profile.steam_id,
                profile.banner_path,
                profile.avatar_path,
                profile.created_at,
                profile.updated_at,
                profile.cached_at,
                profile.expires_at,
                profile.is_backed_up as i32,
                profile.backup_created_at
            ],
        )?;
        Ok(())
    }

    /// Update specific field
    pub fn update_field(conn: &Connection, field: &str, value: Option<&str>) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        
        match field {
            "name" => {
                conn.execute(
                    "UPDATE user_profile SET name = ?1, updated_at = ?2 WHERE id = 1",
                    params![value.unwrap_or("User"), now],
                )?;
            }
            "bio" => {
                conn.execute(
                    "UPDATE user_profile SET bio = ?1, updated_at = ?2 WHERE id = 1",
                    params![value, now],
                )?;
            }
            "steam_id" => {
                conn.execute(
                    "UPDATE user_profile SET steam_id = ?1, updated_at = ?2 WHERE id = 1",
                    params![value, now],
                )?;
            }
            "banner_path" => {
                conn.execute(
                    "UPDATE user_profile SET banner_path = ?1, updated_at = ?2 WHERE id = 1",
                    params![value, now],
                )?;
            }
            "avatar_path" => {
                conn.execute(
                    "UPDATE user_profile SET avatar_path = ?1, updated_at = ?2 WHERE id = 1",
                    params![value, now],
                )?;
            }
            _ => return Err(anyhow::anyhow!("Invalid field name: {}", field)),
        }
        
        Ok(())
    }
}

/// Bypass game operations
pub struct BypassGameOperations;

impl BypassGameOperations {
    /// Insert a bypass game
    pub fn insert(conn: &Connection, game: &BypassGame) -> Result<()> {
        let bypasses_json = serde_json::to_string(&game.bypasses)?;
        
        conn.execute(
            "INSERT OR REPLACE INTO bypass_games 
             (app_id, name, image, bypasses, cached_at, expires_at, last_updated) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                game.app_id,
                game.name,
                game.image,
                bypasses_json,
                game.cached_at,
                game.expires_at,
                game.last_updated
            ],
        )?;
        Ok(())
    }

    /// Get a bypass game by app_id
    pub fn get_by_id(conn: &Connection, app_id: &str) -> Result<Option<BypassGame>> {
        let mut stmt = conn.prepare(
            "SELECT app_id, name, image, bypasses, cached_at, expires_at, last_updated 
             FROM bypass_games WHERE app_id = ?1"
        )?;
        
        let game = stmt.query_row([app_id], |row| BypassGame::from_row(row))
            .optional()?;
        
        Ok(game)
    }

    /// Get all bypass games
    pub fn get_all(conn: &Connection) -> Result<Vec<BypassGame>> {
        let mut games = Vec::new();
        
        let mut stmt = conn.prepare(
            "SELECT app_id, name, image, bypasses, cached_at, expires_at, last_updated 
             FROM bypass_games ORDER BY name"
        )?;
        
        let game_iter = stmt.query_map([], |row| BypassGame::from_row(row))?;
        for game_result in game_iter {
            games.push(game_result?);
        }
        
        Ok(games)
    }

    /// Get expired bypass games
    pub fn get_expired(conn: &Connection) -> Result<Vec<BypassGame>> {
        let now = chrono::Utc::now().timestamp();
        let mut games = Vec::new();
        
        let mut stmt = conn.prepare(
            "SELECT app_id, name, image, bypasses, cached_at, expires_at, last_updated 
             FROM bypass_games WHERE expires_at < ?1"
        )?;
        
        let game_iter = stmt.query_map([now], |row| BypassGame::from_row(row))?;
        for game_result in game_iter {
            games.push(game_result?);
        }
        
        Ok(games)
    }

    /// Clear all bypass games (for refresh)
    pub fn clear_all(conn: &Connection) -> Result<()> {
        conn.execute("DELETE FROM bypass_games", [])?;
        Ok(())
    }

    /// Delete a specific bypass game
    pub fn delete_by_id(conn: &Connection, app_id: &str) -> Result<()> {
        conn.execute("DELETE FROM bypass_games WHERE app_id = ?1", [app_id])?;
        Ok(())
    }

    /// Count total bypass games
    pub fn count(conn: &Connection) -> Result<u32> {
        let count: u32 = conn.query_row(
            "SELECT COUNT(*) FROM bypass_games",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Clean up expired bypass games
    pub fn cleanup_expired(conn: &Connection) -> Result<u32> {
        let now = chrono::Utc::now().timestamp();
        let rows_affected = conn.execute(
            "DELETE FROM bypass_games WHERE expires_at < ?1",
            [now],
        )?;
        Ok(rows_affected as u32)
    }
}
