use serde::{Deserialize, Serialize};
use tauri::command;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SteamUIGame {
    pub appid: u32,
    pub name: String,
    #[serde(rename = "type")]
    pub game_type: String,
    pub schinese_name: String,
    pub isfreeapp: u8,
    pub update_time: String,
    pub change_number: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SteamUIPagination {
    pub page: u32,
    pub limit: u32,
    pub total: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SteamUIResponse {
    pub games: Vec<SteamUIGame>,
    pub page: u32,
    pub has_more: bool,
    pub filter: String,
    pub sort: String,
    pub pagination: SteamUIPagination,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CatalogueGame {
    pub app_id: u32,
    pub name: String,
    pub header_image: String,
    pub game_type: String,
    pub is_free: bool,
    pub update_time: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CatalogueResponse {
    pub games: Vec<CatalogueGame>,
    pub current_page: u32,
    pub total_pages: u32,
    pub total_games: u32,
    pub has_more: bool,
}

/// Fetch games from SteamUI API
#[command]
pub async fn fetch_steamui_games(
    page: Option<u32>,
    search: Option<String>,
    filter: Option<String>,
    sort: Option<String>,
) -> Result<CatalogueResponse, String> {
    let page = page.unwrap_or(1);
    let search = search.unwrap_or_default();
    let filter = filter.unwrap_or_else(|| "game".to_string());
    let sort = sort.unwrap_or_else(|| "update".to_string());

    let url = format!(
        "https://steamui.com/api/loadGames.php?page={}&search={}&filter={}&sort={}",
        page,
        urlencoding::encode(&search),
        filter,
        sort
    );

    println!("🌐 Fetching from SteamUI API: {}", url);

    let client = reqwest::Client::builder()
        .user_agent("Zenith-Launcher/1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("API returned error status: {}", response.status()));
    }

    let steamui_response: SteamUIResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

    // Convert SteamUI games to our catalogue format
    let catalogue_games: Vec<CatalogueGame> = steamui_response
        .games
        .into_iter()
        .map(|game| CatalogueGame {
            app_id: game.appid,
            name: game.name,
            header_image: format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{}/library_hero.jpg",
                game.appid
            ),
            game_type: game.game_type,
            is_free: game.isfreeapp == 1,
            update_time: game.update_time,
        })
        .collect();

    // Since API doesn't provide accurate total count, we'll use has_more flag
    // Calculate estimated total pages based on has_more flag
    let total_pages = if steamui_response.has_more {
        // If has_more is true, we know there are at least (current_page + 1) pages
        page + 1
    } else {
        // If has_more is false, current page is the last page
        page
    };

    println!("✅ Fetched {} games (page {})", catalogue_games.len(), page);

    let games_count = catalogue_games.len() as u32;
    
    Ok(CatalogueResponse {
        games: catalogue_games,
        current_page: page,
        total_pages,
        total_games: if steamui_response.pagination.total > 0 { 
            steamui_response.pagination.total 
        } else { 
            // Estimate based on page and has_more
            if steamui_response.has_more { 
                (page * steamui_response.pagination.limit) + 1 
            } else { 
                (page - 1) * steamui_response.pagination.limit + games_count
            }
        },
        has_more: steamui_response.has_more,
    })
}

/// Search games with debounced functionality
#[command]
pub async fn search_steamui_games(
    query: String,
    page: Option<u32>,
) -> Result<CatalogueResponse, String> {
    if query.trim().is_empty() {
        // If empty query, return regular games
        return fetch_steamui_games(page, None, None, None).await;
    }

    fetch_steamui_games(page, Some(query), Some("game".to_string()), Some("update".to_string())).await
}
