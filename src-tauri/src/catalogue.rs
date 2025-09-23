use lazy_static::lazy_static;
use rusqlite::{Connection, Result, params};
use serde::{Serialize, Deserialize};
use std::sync::Mutex;
use tauri::{command, AppHandle, Manager};
use std::fs;

#[derive(Serialize, Deserialize, Debug)]
pub struct Game {
    app_id: i32,
    name: String,
    genres: String,
    header_image: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PaginatedGames {
    games: Vec<Game>,
    total: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiGame {
    app_id: i32,
    name: String,
    header_image: String,
    genres: String,
    source: String, // "catalogue" or "api"
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HybridSearchResult {
    games: Vec<ApiGame>,
    total: i64,
}


lazy_static! {
    static ref DB_CONNECTION: Mutex<Option<Connection>> = Mutex::new(None);
}

fn get_db_connection() -> std::sync::MutexGuard<'static, Option<Connection>> {
    DB_CONNECTION.lock().unwrap()
}

pub async fn init_database(handle: &AppHandle) -> Result<()> {
    let app_data_dir = handle.path().app_data_dir().expect("Failed to get app data dir");
    if !app_data_dir.exists() {
        fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");
    }
    let db_path = app_data_dir.join("steam_catalogue.db");
    
    println!("Initializing database at: {}", db_path.display());

    let mut conn = Connection::open(&db_path).expect("Failed to open database");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS games (
            app_id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            genres TEXT,
            header_image TEXT
        )",
        [],
    )?;

    let count: i64 = {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM games")?;
        stmt.query_row([], |row| row.get(0))?
    };

    if count == 0 {
        println!("Database is empty. Downloading and populating from CSV...");
        populate_from_github_csv(&mut conn).await.expect("Failed to populate database from GitHub CSV");
        println!("Database population completed successfully!");
    } else {
        println!("Database already populated with {} games.", count);
    }

    *get_db_connection() = Some(conn);

    Ok(())
}

async fn populate_from_github_csv(conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading CSV from GitHub...");
    
    let response = reqwest::get("https://raw.githubusercontent.com/itsBintang/SteamDB/refs/heads/main/SteamDB.csv").await?;
    let csv_content = response.text().await?;
    
    println!("CSV downloaded, parsing data...");
    let mut rdr = csv::Reader::from_reader(csv_content.as_bytes());
    let tx = conn.transaction()?;

    {
        let mut stmt = tx.prepare("INSERT INTO games (app_id, name, genres, header_image) VALUES (?, ?, ?, ?)")?;
        let mut count = 0;
        
        for result in rdr.deserialize() {
            let record: Game = match result {
                Ok(rec) => rec,
                Err(e) => {
                    eprintln!("Skipping problematic CSV row: {}", e);
                    continue;
                }
            };
            
            stmt.execute(params![
                record.app_id,
                record.name,
                record.genres,
                record.header_image
            ])?;
            
            count += 1;
            if count % 1000 == 0 {
                println!("Processed {} games...", count);
            }
        }
        
        println!("Total {} games processed.", count);
    }
    
    tx.commit()?;
    
    println!("Database population complete.");
    Ok(())
}

#[command]
pub fn get_games(page: usize, page_size: usize) -> Result<PaginatedGames, String> {
    let conn_opt = get_db_connection();
    let conn = conn_opt.as_ref().ok_or_else(|| "Database not initialized".to_string())?;
    let offset = (page - 1) * page_size;

    let mut stmt = conn.prepare("SELECT app_id, name, genres, header_image FROM games LIMIT ?1 OFFSET ?2").map_err(|e| e.to_string())?;
    let games_iter = stmt.query_map(params![page_size, offset], |row| {
        Ok(Game {
            app_id: row.get(0)?,
            name: row.get(1)?,
            genres: row.get(2)?,
            header_image: row.get(3)?,
        })
    }).map_err(|e| e.to_string())?;

    let games = games_iter.map(|g| g.unwrap()).collect();

    let mut total_stmt = conn.prepare("SELECT COUNT(*) FROM games").map_err(|e| e.to_string())?;
    let total: i64 = total_stmt.query_row([], |row| row.get(0)).map_err(|e| e.to_string())?;

    Ok(PaginatedGames { games, total })
}

#[command]
pub fn search_games(query: String, page: usize, page_size: usize) -> Result<PaginatedGames, String> {
    let conn_opt = get_db_connection();
    let conn = conn_opt.as_ref().ok_or_else(|| "Database not initialized".to_string())?;
    let offset = (page - 1) * page_size;
    let search_query = format!("%{}%", query);

    let mut stmt = conn.prepare("SELECT app_id, name, genres, header_image FROM games WHERE name LIKE ?1 LIMIT ?2 OFFSET ?3").map_err(|e| e.to_string())?;
    let games_iter = stmt.query_map(params![search_query, page_size, offset], |row| {
        Ok(Game {
            app_id: row.get(0)?,
            name: row.get(1)?,
            genres: row.get(2)?,
            header_image: row.get(3)?,
        })
    }).map_err(|e| e.to_string())?;

    let games = games_iter.map(|g| g.unwrap()).collect();

    let mut total_stmt = conn.prepare("SELECT COUNT(*) FROM games WHERE name LIKE ?1").map_err(|e| e.to_string())?;
    let total: i64 = total_stmt.query_row(params![search_query], |row| row.get(0)).map_err(|e| e.to_string())?;

    Ok(PaginatedGames { games, total })
}

#[command]
pub fn get_all_genres() -> Result<Vec<String>, String> {
    let conn_opt = get_db_connection();
    let conn = conn_opt.as_ref().ok_or_else(|| "Database not initialized".to_string())?;
    let mut stmt = conn.prepare("SELECT DISTINCT genres FROM games").map_err(|e| e.to_string())?;
    let genre_iter = stmt.query_map([], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?;

    let mut all_genres = std::collections::HashSet::new();
    for genres_str in genre_iter {
        for genre in genres_str.unwrap().split(',') {
            if !genre.trim().is_empty() {
                all_genres.insert(genre.trim().to_string());
            }
        }
    }
    let mut sorted_genres: Vec<String> = all_genres.into_iter().collect();
    sorted_genres.sort();
    Ok(sorted_genres)
}

#[command]
pub fn filter_games_by_genre(genres: Vec<String>, page: usize, page_size: usize) -> Result<PaginatedGames, String> {
    let conn_opt = get_db_connection();
    let conn = conn_opt.as_ref().ok_or_else(|| "Database not initialized".to_string())?;
    let offset = (page - 1) * page_size;

    if genres.is_empty() {
        // If no genres selected, return all games
        return get_games(page, page_size);
    }

    // Build WHERE clause for genre filtering
    let mut where_conditions = Vec::new();
    for _genre in &genres {
        where_conditions.push("genres LIKE ?".to_string());
    }
    let where_clause = where_conditions.join(" AND ");

    let select_query = format!(
        "SELECT app_id, name, genres, header_image FROM games WHERE {} LIMIT ? OFFSET ?",
        where_clause
    );
    let count_query = format!(
        "SELECT COUNT(*) FROM games WHERE {}",
        where_clause
    );

    // Prepare parameters
    let mut params: Vec<String> = Vec::new();
    for genre in &genres {
        params.push(format!("%{}%", genre));
    }

    // Execute select query
    let mut stmt = conn.prepare(&select_query).map_err(|e| e.to_string())?;
    let mut bind_params: Vec<&dyn rusqlite::ToSql> = Vec::new();
    for param in &params {
        bind_params.push(param);
    }
    bind_params.push(&page_size);
    bind_params.push(&offset);

    let games_iter = stmt.query_map(&bind_params[..], |row| {
        Ok(Game {
            app_id: row.get(0)?,
            name: row.get(1)?,
            genres: row.get(2)?,
            header_image: row.get(3)?,
        })
    }).map_err(|e| e.to_string())?;

    let games = games_iter.map(|g| g.unwrap()).collect();

    // Execute count query
    let mut count_stmt = conn.prepare(&count_query).map_err(|e| e.to_string())?;
    let count_params: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
    let total: i64 = count_stmt.query_row(&count_params[..], |row| row.get(0)).map_err(|e| e.to_string())?;

    Ok(PaginatedGames { games, total })
}

#[command]
pub async fn hybrid_search(query: String, page: usize, page_size: usize) -> Result<HybridSearchResult, String> {
    // For now, hybrid search only works for the first page to keep it simple.
    // Pagination beyond page 1 will only search the local catalogue.
    if page > 1 {
        let catalogue_result = search_games(query, page, page_size)?;
        let api_games = catalogue_result.games.into_iter().map(|g| ApiGame {
            app_id: g.app_id,
            name: g.name,
            header_image: g.header_image,
            genres: g.genres,
            source: "catalogue".to_string(),
        }).collect();

        return Ok(HybridSearchResult {
            games: api_games,
            total: catalogue_result.total,
        });
    }

    // --- Page 1 Logic ---
    let local_result = search_games(query.clone(), 1, page_size)?;
    let catalogue_count = local_result.total;
    let mut combined_games: Vec<ApiGame> = local_result.games.into_iter().map(|g| ApiGame {
        app_id: g.app_id,
        name: g.name,
        header_image: g.header_image,
        genres: g.genres,
        source: "catalogue".to_string(),
    }).collect();

    let remaining_slots = if combined_games.len() < page_size {
        page_size - combined_games.len()
    } else {
        0
    };
    
    let mut api_games_vec: Vec<ApiGame> = Vec::new();
    if remaining_slots > 0 {
        if let Ok(external_games) = search_external_api(&query, remaining_slots).await {
            for game in external_games {
                // Avoid duplicates
                if !combined_games.iter().any(|g| g.app_id == game.app_id) {
                    api_games_vec.push(game);
                }
            }
        }
    }
    
    let api_count = api_games_vec.len() as i64;
    combined_games.extend(api_games_vec);

    Ok(HybridSearchResult {
        games: combined_games,
        total: catalogue_count + api_count,
    })
}

async fn search_external_api(query: &str, limit: usize) -> Result<Vec<ApiGame>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "https://store.steampowered.com/api/storesearch/?term={}&l=english&cc=US",
        urlencoding::encode(query)
    );

    let response = reqwest::get(&url).await?;

    if !response.status().is_success() {
        return Err(format!("API request failed with status: {}", response.status()).into());
    }

    let json: serde_json::Value = response.json().await?;
    let mut results = Vec::new();

    if let Some(items) = json["items"].as_array() {
        for item in items.iter().take(limit) {
            if let (Some(id), Some(name), Some(img)) = (
                item["id"].as_u64(),
                item["name"].as_str(),
                item["tiny_image"].as_str(),
            ) {
                let item_type = item["type"].as_str().unwrap_or("");
                
                // Filter out non-game items (DLC, soundtracks, etc.) similar to original implementation
                if !is_valid_game_item(name, item_type) {
                    continue;
                }

                results.push(ApiGame {
                    app_id: id as i32,
                    name: name.to_string(),
                    header_image: format!("https://cdn.akamai.steamstatic.com/steam/apps/{}/library_hero.jpg", id),
                    genres: "".to_string(), // External API doesn't provide genres
                    source: "api".to_string(),
                });
            }
        }
    }

    Ok(results)
}

fn is_valid_game_item(name: &str, item_type: &str) -> bool {
    // Filter out non-game item types
    if item_type != "app" {
        return false;
    }

    let name_lower = name.to_lowercase();

    // Filter out common DLC/addon patterns
    let excluded_patterns = [
        "dlc", "downloadable content", "expansion pack", "season pass",
        "soundtrack", "ost", "original soundtrack", "music",
        "artbook", "art book", "sketchbook", "wallpaper", "avatar", "emoticon",
        "trading card", "badge", "profile background",
        "demo", "beta", "test", "benchmark",
        "tool", "sdk", "editor", "mod",
        "- soundtrack", "- ost", "- music",
        "supporter pack", "cosmetic pack", "skin pack",
        "character pack", "weapon pack", "map pack",
        "content pack", "bonus content", "digital deluxe",
        "collector's edition upgrade", "upgrade pack",
        "companion app", "mobile companion", "viewer",
        "prologue", "prelude", "epilogue", "chapter",
        "free content", "free dlc", "update", "starter pack"
    ];

    // Check if name contains any excluded patterns
    for pattern in &excluded_patterns {
        if name_lower.contains(pattern) {
            return false;
        }
    }

    // Additional filtering for items with " - " separator
    if name_lower.contains(" - ") && (
        name_lower.contains("pack") ||
        name_lower.contains("bundle") ||
        name_lower.contains("edition") ||
        name_lower.contains("content")
    ) {
        // Allow some exceptions like "Game - Complete Edition" which might be the main game
        if !name_lower.contains("complete") && !name_lower.contains("definitive") &&
           !name_lower.contains("ultimate") && !name_lower.contains("goty") &&
           !name_lower.contains("game of the year") {
            return false;
        }
    }

    true
}


// Note: A more complex filtering function would be needed for combining genre/tag filters.
// For now, we'll keep it simple and let the frontend do multi-filtering on the fetched genres/tags list.
