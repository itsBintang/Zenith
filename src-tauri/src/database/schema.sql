-- SQLite Database Schema for Zenith Launcher
-- Based on actual GameDetail struct and LibraryGame struct

-- Games basic info (from LibraryGame struct)
CREATE TABLE games (
    app_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    header_image TEXT,
    cached_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    last_updated INTEGER DEFAULT (strftime('%s', 'now'))
);

-- Detailed game information (from GameDetail struct) with granular TTL
CREATE TABLE game_details (
    app_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    header_image TEXT,
    banner_image TEXT,
    detailed_description TEXT,
    release_date TEXT,
    publisher TEXT,
    trailer TEXT, -- Optional trailer URL
    screenshots TEXT, -- JSON array of screenshot URLs
    sysreq_min TEXT, -- JSON array of (String, String) tuples for minimum requirements
    sysreq_rec TEXT, -- JSON array of (String, String) tuples for recommended requirements
    pc_requirements TEXT, -- JSON blob for PcRequirements struct (minimum/recommended)
    dlc TEXT, -- JSON array of DLC AppIDs
    drm_notice TEXT, -- Optional DRM information
    
    -- Global cache timestamps (for backward compatibility)
    cached_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    last_updated INTEGER DEFAULT (strftime('%s', 'now')),
    
    -- Granular expiry timestamps for different data categories
    dynamic_expires_at INTEGER NOT NULL,    -- For DLC list (3 days)
    semistatic_expires_at INTEGER NOT NULL, -- For name, images, trailer (3 weeks)
    static_expires_at INTEGER NOT NULL,     -- For screenshots, descriptions, sysreq (60+ days)
    
    FOREIGN KEY (app_id) REFERENCES games(app_id) ON DELETE CASCADE
);

-- User library tracking (for My Library feature)
CREATE TABLE user_library (
    app_id TEXT PRIMARY KEY,
    added_at INTEGER DEFAULT (strftime('%s', 'now')),
    last_accessed INTEGER,
    access_count INTEGER DEFAULT 0,
    FOREIGN KEY (app_id) REFERENCES games(app_id) ON DELETE CASCADE
);

-- Cache metadata and application settings
CREATE TABLE cache_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER DEFAULT (strftime('%s', 'now'))
);

-- User profile data with TTL and backup support
CREATE TABLE user_profile (
    id INTEGER PRIMARY KEY DEFAULT 1, -- Single profile entry
    name TEXT NOT NULL DEFAULT 'User',
    bio TEXT DEFAULT 'Steam User',
    steam_id TEXT,
    banner_path TEXT, -- Local file path to banner image
    avatar_path TEXT, -- Local file path to avatar image
    created_at INTEGER DEFAULT 0,
    updated_at INTEGER DEFAULT 0,
    cached_at INTEGER DEFAULT 0,
    expires_at INTEGER DEFAULT 0, -- TTL will be set programmatically
    is_backed_up INTEGER DEFAULT 0, -- Backup flag
    backup_created_at INTEGER DEFAULT 0
);

-- Profile backup table for recovery
CREATE TABLE user_profile_backup (
    id INTEGER PRIMARY KEY DEFAULT 1,
    name TEXT NOT NULL DEFAULT 'User',
    bio TEXT DEFAULT 'Steam User', 
    steam_id TEXT,
    banner_path TEXT,
    avatar_path TEXT,
    created_at INTEGER DEFAULT 0,
    updated_at INTEGER DEFAULT 0,
    cached_at INTEGER DEFAULT 0,
    expires_at INTEGER DEFAULT 0,
    backup_created_at INTEGER DEFAULT 0,
    backup_reason TEXT DEFAULT 'manual' -- migration, corruption, manual, etc.
);

-- Bypass games data (static data with monthly TTL)
CREATE TABLE bypass_games (
    app_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    image TEXT NOT NULL,
    bypasses TEXT NOT NULL, -- JSON array of bypass info (type, url)
    cached_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL, -- 1 month TTL for static data
    last_updated INTEGER DEFAULT (strftime('%s', 'now'))
);

-- Indexes for better query performance
CREATE INDEX idx_games_name ON games(name);
CREATE INDEX idx_games_cached_at ON games(cached_at);
CREATE INDEX idx_games_expires_at ON games(expires_at);

CREATE INDEX idx_game_details_cached_at ON game_details(cached_at);
CREATE INDEX idx_game_details_expires_at ON game_details(expires_at);
CREATE INDEX idx_game_details_name ON game_details(name);

-- Indexes for granular TTL queries
CREATE INDEX idx_game_details_dynamic_expires ON game_details(dynamic_expires_at);
CREATE INDEX idx_game_details_semistatic_expires ON game_details(semistatic_expires_at);
CREATE INDEX idx_game_details_static_expires ON game_details(static_expires_at);

CREATE INDEX idx_user_library_added_at ON user_library(added_at);
CREATE INDEX idx_user_library_last_accessed ON user_library(last_accessed);

CREATE INDEX idx_cache_metadata_key ON cache_metadata(key);
CREATE INDEX idx_user_profile_updated_at ON user_profile(updated_at);

-- Indexes for bypass games
CREATE INDEX idx_bypass_games_name ON bypass_games(name);
CREATE INDEX idx_bypass_games_cached_at ON bypass_games(cached_at);
CREATE INDEX idx_bypass_games_expires_at ON bypass_games(expires_at);

-- Insert initial metadata
INSERT INTO cache_metadata (key, value) VALUES 
    ('schema_version', '1'),
    ('created_at', strftime('%s', 'now')),
    ('last_cleanup', '0');

-- Insert default user profile
INSERT INTO user_profile (id, name, bio) VALUES (1, 'User', 'Steam User');
