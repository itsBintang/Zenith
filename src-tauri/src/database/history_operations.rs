use rusqlite::{params, Connection, Result};
use super::history_models::{DownloadHistoryEntry, DownloadHistorySummary, HistoryStats};
use chrono::Utc;

pub struct DownloadHistoryOperations;

impl DownloadHistoryOperations {
    /// Add new download to history
    pub fn add_download(conn: &Connection, entry: &DownloadHistoryEntry) -> Result<i64> {
        let sql = r#"
            INSERT INTO download_history (
                download_id, download_type, source_type, url, file_name, file_size, save_path,
                app_id, game_name, final_progress, download_speed_avg, total_time_seconds,
                status, error_message, started_at, completed_at, user_agent, headers,
                is_redownloadable, original_request
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20
            )
        "#;

        conn.execute(
            sql,
            params![
                entry.download_id,
                entry.download_type,
                entry.source_type,
                entry.url,
                entry.file_name,
                entry.file_size,
                entry.save_path,
                entry.app_id,
                entry.game_name,
                entry.final_progress,
                entry.download_speed_avg,
                entry.total_time_seconds,
                entry.status,
                entry.error_message,
                entry.started_at,
                entry.completed_at,
                entry.user_agent,
                entry.headers,
                entry.is_redownloadable,
                entry.original_request,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Update download status and completion info
    pub fn update_download_completion(
        conn: &Connection,
        download_id: &str,
        status: &str,
        final_progress: f64,
        avg_speed: i64,
        total_time: i64,
        file_size: Option<i64>,
        error_message: Option<&str>,
    ) -> Result<()> {
        let sql = r#"
            UPDATE download_history 
            SET status = ?2, 
                final_progress = ?3, 
                download_speed_avg = ?4, 
                total_time_seconds = ?5,
                file_size = COALESCE(?6, file_size),
                error_message = ?7,
                completed_at = ?8
            WHERE download_id = ?1
        "#;

        conn.execute(
            sql,
            params![
                download_id,
                status,
                final_progress,
                avg_speed,
                total_time,
                file_size,
                error_message,
                Utc::now().timestamp(),
            ],
        )?;

        Ok(())
    }

    /// Get download history summary with pagination
    pub fn get_history_summary(
        conn: &Connection,
        limit: Option<u32>,
        offset: Option<u32>,
        filter_type: Option<&str>, // 'bypass', 'regular', or None for all
    ) -> Result<Vec<DownloadHistorySummary>> {
        let mut sql = "SELECT * FROM download_history_summary".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(filter) = filter_type {
            sql.push_str(" WHERE download_type = ?");
            params.push(Box::new(filter.to_string()));
        }

        sql.push_str(" ORDER BY completed_at DESC");

        if let Some(limit_val) = limit {
            sql.push_str(" LIMIT ?");
            params.push(Box::new(limit_val));

            if let Some(offset_val) = offset {
                sql.push_str(" OFFSET ?");
                params.push(Box::new(offset_val));
            }
        }

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        
        let history_iter = stmt.query_map(&param_refs[..], |row| {
            DownloadHistorySummary::from_row(row)
        })?;

        let mut history = Vec::new();
        for entry in history_iter {
            history.push(entry?);
        }

        Ok(history)
    }

    /// Get full download history entry by ID
    pub fn get_download_by_id(conn: &Connection, id: i64) -> Result<Option<DownloadHistoryEntry>> {
        let sql = "SELECT * FROM download_history WHERE id = ?1";
        
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query_map(params![id], |row| {
            DownloadHistoryEntry::from_row(row)
        })?;

        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    /// Get download history entry by download_id
    pub fn get_download_by_download_id(conn: &Connection, download_id: &str) -> Result<Option<DownloadHistoryEntry>> {
        let sql = "SELECT * FROM download_history WHERE download_id = ?1";
        
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query_map(params![download_id], |row| {
            DownloadHistoryEntry::from_row(row)
        })?;

        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    /// Get history statistics
    pub fn get_history_stats(conn: &Connection) -> Result<HistoryStats> {
        let sql = r#"
            SELECT 
                COUNT(*) as total_downloads,
                COUNT(CASE WHEN status = 'completed' THEN 1 END) as completed_downloads,
                COUNT(CASE WHEN status = 'failed' THEN 1 END) as failed_downloads,
                COALESCE(SUM(CASE WHEN status = 'completed' THEN file_size END), 0) / 1073741824.0 as total_data_gb,
                COUNT(CASE WHEN download_type = 'bypass' THEN 1 END) as bypass_downloads,
                COUNT(CASE WHEN download_type = 'regular' THEN 1 END) as regular_downloads,
                COALESCE(AVG(CASE WHEN status = 'completed' AND download_speed_avg > 0 THEN download_speed_avg END), 0) / 1048576.0 as avg_speed_mbps,
                COALESCE(SUM(CASE WHEN status = 'completed' THEN total_time_seconds END), 0) / 3600.0 as total_time_hours
            FROM download_history
        "#;

        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query_map([], |row| {
            Ok(HistoryStats {
                total_downloads: row.get("total_downloads")?,
                completed_downloads: row.get("completed_downloads")?,
                failed_downloads: row.get("failed_downloads")?,
                total_data_downloaded_gb: row.get("total_data_gb")?,
                bypass_downloads: row.get("bypass_downloads")?,
                regular_downloads: row.get("regular_downloads")?,
                avg_download_speed_mbps: row.get("avg_speed_mbps")?,
                total_download_time_hours: row.get("total_time_hours")?,
            })
        })?;

        if let Some(row) = rows.next() {
            Ok(row?)
        } else {
            Ok(HistoryStats {
                total_downloads: 0,
                completed_downloads: 0,
                failed_downloads: 0,
                total_data_downloaded_gb: 0.0,
                bypass_downloads: 0,
                regular_downloads: 0,
                avg_download_speed_mbps: 0.0,
                total_download_time_hours: 0.0,
            })
        }
    }

    /// Delete history entry
    pub fn delete_history_entry(conn: &Connection, id: i64) -> Result<bool> {
        let sql = "DELETE FROM download_history WHERE id = ?1";
        let affected = conn.execute(sql, params![id])?;
        Ok(affected > 0)
    }

    /// Clear all history (with optional filter)
    pub fn clear_history(conn: &Connection, filter_type: Option<&str>) -> Result<u32> {
        let sql = if let Some(filter) = filter_type {
            "DELETE FROM download_history WHERE download_type = ?1"
        } else {
            "DELETE FROM download_history"
        };

        let affected = if let Some(filter) = filter_type {
            conn.execute(sql, params![filter])?
        } else {
            conn.execute(sql, [])?
        };

        Ok(affected as u32)
    }

    /// Search history by game name or file name
    pub fn search_history(
        conn: &Connection,
        search_term: &str,
        limit: Option<u32>,
    ) -> Result<Vec<DownloadHistorySummary>> {
        let sql = r#"
            SELECT * FROM download_history_summary 
            WHERE (file_name LIKE ?1 OR game_name LIKE ?1 OR app_id LIKE ?1)
            ORDER BY completed_at DESC
            LIMIT ?2
        "#;

        let search_pattern = format!("%{}%", search_term);
        let limit_val = limit.unwrap_or(50);

        let mut stmt = conn.prepare(sql)?;
        let history_iter = stmt.query_map(params![search_pattern, limit_val], |row| {
            DownloadHistorySummary::from_row(row)
        })?;

        let mut history = Vec::new();
        for entry in history_iter {
            history.push(entry?);
        }

        Ok(history)
    }
}
