-- Download History Table
CREATE TABLE IF NOT EXISTS download_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    download_id TEXT NOT NULL,           -- Actual download ID from aria2/torrent
    download_type TEXT NOT NULL,         -- 'bypass' or 'regular'
    source_type TEXT NOT NULL,           -- 'bypass', 'manual', 'game_download', etc.
    
    -- Download Details
    url TEXT NOT NULL,
    file_name TEXT,
    file_size INTEGER DEFAULT 0,
    save_path TEXT NOT NULL,
    
    -- Game/Bypass specific
    app_id TEXT,                         -- Steam App ID (for bypass downloads)
    game_name TEXT,                      -- Game name (for bypass downloads)
    
    -- Progress & Status
    final_progress REAL DEFAULT 0.0,    -- Final progress (0.0 to 1.0)
    download_speed_avg INTEGER DEFAULT 0, -- Average download speed in bytes/s
    total_time_seconds INTEGER DEFAULT 0, -- Total download time
    
    -- Status & Result
    status TEXT NOT NULL,                -- 'completed', 'cancelled', 'failed'
    error_message TEXT,                  -- Error message if failed
    
    -- Timestamps
    started_at DATETIME NOT NULL,
    completed_at DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    
    -- Metadata
    user_agent TEXT,
    headers TEXT,                        -- JSON string of headers used
    
    -- Re-download capability
    is_redownloadable BOOLEAN DEFAULT 1, -- Can this be re-downloaded?
    original_request TEXT                -- JSON of original download request for re-download
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_download_history_type ON download_history(download_type);
CREATE INDEX IF NOT EXISTS idx_download_history_app_id ON download_history(app_id);
CREATE INDEX IF NOT EXISTS idx_download_history_status ON download_history(status);
CREATE INDEX IF NOT EXISTS idx_download_history_completed_at ON download_history(completed_at);
CREATE INDEX IF NOT EXISTS idx_download_history_download_id ON download_history(download_id);

-- View for easy querying
CREATE VIEW IF NOT EXISTS download_history_summary AS
SELECT 
    id,
    download_type,
    source_type,
    file_name,
    ROUND(file_size / 1048576.0, 2) as file_size_mb,
    app_id,
    game_name,
    status,
    ROUND(final_progress * 100, 1) as progress_percent,
    ROUND(download_speed_avg / 1048576.0, 2) as avg_speed_mbps,
    total_time_seconds,
    started_at,
    completed_at,
    is_redownloadable
FROM download_history
ORDER BY completed_at DESC;
