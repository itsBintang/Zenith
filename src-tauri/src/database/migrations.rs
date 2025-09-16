use anyhow::Result;
use rusqlite::Connection;

/// Current database schema version
const CURRENT_SCHEMA_VERSION: i32 = 3;

/// Run all necessary database migrations
pub fn run_migrations(conn: &Connection) -> Result<()> {
    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    
    let current_version = get_schema_version(conn)?;
    
    if current_version < CURRENT_SCHEMA_VERSION {
        println!("Migrating database from version {} to {}", current_version, CURRENT_SCHEMA_VERSION);
        
        // Run migrations step by step
        for version in (current_version + 1)..=CURRENT_SCHEMA_VERSION {
            migrate_to_version(conn, version)?;
        }
        
        // Update schema version
        set_schema_version(conn, CURRENT_SCHEMA_VERSION)?;
        println!("Database migration completed successfully");
    }
    
    Ok(())
}

/// Get current schema version from database
fn get_schema_version(conn: &Connection) -> Result<i32> {
    // Check if cache_metadata table exists
    let table_exists: bool = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='cache_metadata'")?
        .exists([])?;
    
    if !table_exists {
        return Ok(0); // No schema exists yet
    }
    
    // Try to get schema version
    match conn.query_row(
        "SELECT value FROM cache_metadata WHERE key = 'schema_version'",
        [],
        |row| {
            let value: String = row.get(0)?;
            Ok(value.parse::<i32>().unwrap_or(0))
        },
    ) {
        Ok(version) => Ok(version),
        Err(_) => Ok(0), // No version found, assume 0
    }
}

/// Set schema version in database
fn set_schema_version(conn: &Connection, version: i32) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO cache_metadata (key, value, updated_at) VALUES ('schema_version', ?1, strftime('%s', 'now'))",
        [version.to_string()],
    )?;
    Ok(())
}

/// Migrate to specific version
fn migrate_to_version(conn: &Connection, version: i32) -> Result<()> {
    match version {
        1 => migrate_to_v1(conn),
        2 => migrate_to_v2(conn),
        3 => migrate_to_v3(conn),
        _ => Err(anyhow::anyhow!("Unknown migration version: {}", version)),
    }
}

/// Initial schema creation (version 1)
fn migrate_to_v1(conn: &Connection) -> Result<()> {
    println!("Creating initial database schema (v1)...");
    
    // Read and execute the schema SQL
    let schema_sql = include_str!("schema.sql");
    conn.execute_batch(schema_sql)?;
    
    println!("Initial schema created successfully");
    Ok(())
}

/// Add granular TTL support (version 2)
fn migrate_to_v2(conn: &Connection) -> Result<()> {
    println!("Migrating to granular TTL schema (v2)...");
    
    // Check if columns already exist before adding them
    let mut stmt = conn.prepare("PRAGMA table_info(game_details)")?;
    let column_names: Vec<String> = stmt.query_map([], |row| {
        Ok(row.get::<_, String>(1)?) // column name is at index 1
    })?.collect::<Result<Vec<_>, _>>()?;
    
    // Add granular TTL columns to game_details table if they don't exist
    if !column_names.contains(&"dynamic_expires_at".to_string()) {
        conn.execute(
            "ALTER TABLE game_details ADD COLUMN dynamic_expires_at INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    
    if !column_names.contains(&"semistatic_expires_at".to_string()) {
        conn.execute(
            "ALTER TABLE game_details ADD COLUMN semistatic_expires_at INTEGER NOT NULL DEFAULT 0", 
            [],
        )?;
    }
    
    if !column_names.contains(&"static_expires_at".to_string()) {
        conn.execute(
            "ALTER TABLE game_details ADD COLUMN static_expires_at INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    
    // Create indexes for new granular TTL columns (ignore if they already exist)
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_game_details_dynamic_expires ON game_details(dynamic_expires_at)",
        [],
    );
    
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_game_details_semistatic_expires ON game_details(semistatic_expires_at)",
        [],
    );
    
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_game_details_static_expires ON game_details(static_expires_at)",
        [],
    );
    
    // Update existing records with appropriate TTL values
    let now = chrono::Utc::now().timestamp();
    
    // Import TTL config
    use crate::database::ttl_config::TtlConfig;
    
    conn.execute(
        "UPDATE game_details SET 
         dynamic_expires_at = ?1,
         semistatic_expires_at = ?2,
         static_expires_at = ?3
         WHERE dynamic_expires_at = 0",
        [
            now + TtlConfig::DLC_LIST,
            now + TtlConfig::GAME_NAME, 
            now + TtlConfig::SCREENSHOTS,
        ],
    )?;
    
    println!("Granular TTL schema migration completed successfully");
    Ok(())
}

/// Migration to version 3: Add user profile table
fn migrate_to_v3(conn: &Connection) -> Result<()> {
    println!("Adding user profile table (v3)...");
    
    // Check if user_profile table already exists
    let table_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='user_profile'",
        [],
        |row| Ok(row.get::<_, i32>(0)? > 0)
    )?;
    
    if !table_exists {
        // Create user_profile table
        conn.execute(
            "CREATE TABLE user_profile (
                id INTEGER PRIMARY KEY DEFAULT 1,
                name TEXT NOT NULL DEFAULT 'Nazril',
                bio TEXT DEFAULT 'Steam User',
                steam_id TEXT,
                banner_path TEXT,
                avatar_path TEXT,
                created_at INTEGER DEFAULT (strftime('%s', 'now')),
                updated_at INTEGER DEFAULT (strftime('%s', 'now'))
            )",
            [],
        )?;
        
        // Create index for user_profile
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_user_profile_updated_at ON user_profile(updated_at)",
            [],
        );
        
        // Insert default user profile
        conn.execute(
            "INSERT INTO user_profile (id, name, bio) VALUES (1, 'User', 'Steam User')",
            [],
        )?;
    }
    
    println!("User profile table migration completed successfully");
    Ok(())
}

/// Check database integrity
pub fn check_database_integrity(conn: &Connection) -> Result<bool> {
    let integrity_result: String = conn.query_row("PRAGMA integrity_check", [], |row| {
        row.get(0)
    })?;
    
    Ok(integrity_result == "ok")
}

/// Get database info for debugging
pub fn get_database_info(conn: &Connection) -> Result<DatabaseInfo> {
    let schema_version = get_schema_version(conn)?;
    
    let page_count: i64 = conn.query_row("PRAGMA page_count", [], |row| row.get(0))?;
    let page_size: i64 = conn.query_row("PRAGMA page_size", [], |row| row.get(0))?;
    let db_size = page_count * page_size;
    
    let journal_mode: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    let synchronous: String = conn.query_row("PRAGMA synchronous", [], |row| row.get(0))?;
    let foreign_keys: bool = conn.query_row("PRAGMA foreign_keys", [], |row| {
        let val: i64 = row.get(0)?;
        Ok(val == 1)
    })?;
    
    // Get table list
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
    let table_rows = stmt.query_map([], |row| {
        let name: String = row.get(0)?;
        Ok(name)
    })?;
    
    let mut tables = Vec::new();
    for table_result in table_rows {
        tables.push(table_result?);
    }
    
    Ok(DatabaseInfo {
        schema_version,
        db_size_bytes: db_size,
        page_count,
        page_size,
        journal_mode,
        synchronous,
        foreign_keys_enabled: foreign_keys,
        tables,
    })
}

/// Database information structure
#[derive(Debug)]
pub struct DatabaseInfo {
    pub schema_version: i32,
    pub db_size_bytes: i64,
    pub page_count: i64,
    pub page_size: i64,
    pub journal_mode: String,
    pub synchronous: String,
    pub foreign_keys_enabled: bool,
    pub tables: Vec<String>,
}

impl std::fmt::Display for DatabaseInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Database Information:")?;
        writeln!(f, "  Schema Version: {}", self.schema_version)?;
        writeln!(f, "  Size: {:.2} MB ({} pages × {} bytes)", 
                 self.db_size_bytes as f64 / 1024.0 / 1024.0, 
                 self.page_count, 
                 self.page_size)?;
        writeln!(f, "  Journal Mode: {}", self.journal_mode)?;
        writeln!(f, "  Synchronous: {}", self.synchronous)?;
        writeln!(f, "  Foreign Keys: {}", if self.foreign_keys_enabled { "ON" } else { "OFF" })?;
        writeln!(f, "  Tables: {}", self.tables.join(", "))?;
        Ok(())
    }
}
