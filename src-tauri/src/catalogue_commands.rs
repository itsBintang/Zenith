use crate::hydra_api::{HYDRA_API, PaginatedCatalogueResponse, SearchFilters};
use serde::{Deserialize, Serialize};
use tauri::command;

// Frontend-facing data structures
#[derive(Debug, Serialize, Deserialize)]
pub struct CatalogueGame {
    pub id: String,
    pub object_id: String,
    pub title: String,
    pub shop: String,
    pub genres: Vec<String>,
    pub library_image_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CatalogueListResponse {
    pub games: Vec<CatalogueGame>,
    pub total_count: u32,
}

// Convert from Hydra API format to our format
impl From<crate::hydra_api::HydraGame> for CatalogueGame {
    fn from(hydra_game: crate::hydra_api::HydraGame) -> Self {
        Self {
            id: hydra_game.id,
            object_id: hydra_game.object_id,
            title: hydra_game.title,
            shop: hydra_game.shop,
            genres: hydra_game.genres,
            library_image_url: hydra_game.library_image_url,
        }
    }
}

/// Get catalogue list (all games)
/// Based on your hydra-test catalogue-list.js implementation
#[command]
pub async fn get_catalogue_list(
    limit: Option<u32>, 
    offset: Option<u32>
) -> Result<CatalogueListResponse, String> {
    println!("🎮 [Command] Getting catalogue list...");
    
    match HYDRA_API.get_catalogue_list(limit, offset).await {
        Ok(response) => {
            let games: Vec<CatalogueGame> = response
                .edges
                .into_iter()
                .map(CatalogueGame::from)
                .collect();

            Ok(CatalogueListResponse {
                games,
                total_count: response.count,
            })
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch catalogue list: {}", e);
            Err(format!("Failed to fetch catalogue list: {}", e))
        }
    }
}

/// Get paginated catalogue with filters (like Hydra's pagination)
#[command]
pub async fn get_paginated_catalogue(
    page: u32,
    items_per_page: Option<u32>,
    genres: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    developers: Option<Vec<String>>,
    publishers: Option<Vec<String>>,
) -> Result<PaginatedCatalogueResponse, String> {
    // Fetching catalogue data
    
    let items_per_page = items_per_page.unwrap_or(20);
    
    // Create filter payload like Hydra
    let filters = crate::hydra_api::SearchFilters {
        title: String::new(), // Empty for pagination
        genres: genres.clone().unwrap_or_default(),
        tags: tags.clone().unwrap_or_default(),
        developers: developers.clone().unwrap_or_default(),
        publishers: publishers.clone().unwrap_or_default(),
        download_source_fingerprints: Vec::new(), // Not used for now
    };
    
    println!("🔧 Backend received filters:");
    println!("   genres: {:?}", genres);
    println!("   tags: {:?}", tags);
    println!("   developers: {:?}", developers);
    println!("   publishers: {:?}", publishers);

    match HYDRA_API.search_games_paginated_with_filters(filters, page, items_per_page).await {
        Ok(response) => {
            // Debug: Log first game data being sent to frontend
            if let Some(first_game) = response.games.first() {
                println!(
                    "🔍 First game being sent to frontend: title='{}', objectId='{}', libraryImageUrl='{:?}'",
                    first_game.title, first_game.object_id, first_game.library_image_url
                );
            }
            Ok(response)
        },
        Err(e) => {
            eprintln!("❌ Failed to fetch paginated catalogue: {}", e);
            Err(format!("Failed to fetch paginated catalogue: {}", e))
        }
    }
}

/// Search games in catalogue with pagination
#[command]
pub async fn search_catalogue_games(
    query: String,
    page: Option<u32>,
    items_per_page: Option<u32>,
    genres: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    developers: Option<Vec<String>>,
    publishers: Option<Vec<String>>,
) -> Result<PaginatedCatalogueResponse, String> {
    // Searching catalogue games
    
    let page = page.unwrap_or(1);
    let items_per_page = items_per_page.unwrap_or(20);
    
    // Create filter payload like Hydra
    let filters = SearchFilters {
        title: query.clone(),
        genres: genres.clone().unwrap_or_default(),
        tags: tags.clone().unwrap_or_default(),
        developers: developers.clone().unwrap_or_default(),
        publishers: publishers.clone().unwrap_or_default(),
        download_source_fingerprints: Vec::new(), // Not used for now
    };
    
    println!("🔧 Search backend received:");
    println!("   query: {:?}", query);
    println!("   genres: {:?}", genres);
    println!("   tags: {:?}", tags);
    println!("   developers: {:?}", developers);
    println!("   publishers: {:?}", publishers);

    match HYDRA_API.search_games_paginated_with_filters(filters, page, items_per_page).await {
        Ok(response) => {
            // Debug: Log first search result
            if let Some(first_game) = response.games.first() {
                println!(
                    "🔍 First search result: title='{}', objectId='{}', libraryImageUrl='{:?}'",
                    first_game.title, first_game.object_id, first_game.library_image_url
                );
            }
            Ok(response)
        },
        Err(e) => {
            eprintln!("❌ Failed to search catalogue games: {}", e);
            Err(format!("Failed to search catalogue games: {}", e))
        }
    }
}

/// Get sample games for quick preview
#[command]
pub async fn get_sample_catalogue_games(limit: Option<u32>) -> Result<Vec<CatalogueGame>, String> {
    // Getting sample games
    
    let limit = limit.unwrap_or(10);
    
    match HYDRA_API.get_catalogue_list(Some(limit), Some(0)).await {
        Ok(response) => {
            let games: Vec<CatalogueGame> = response
                .edges
                .into_iter()
                .map(CatalogueGame::from)
                .collect();

            println!("✅ Retrieved {} sample games", games.len());
            Ok(games)
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch sample games: {}", e);
            Err(format!("Failed to fetch sample games: {}", e))
        }
    }
}

/// Test Hydra API connection
#[command]
pub async fn test_hydra_connection() -> Result<String, String> {
    // Testing API connection
    
    match HYDRA_API.get_catalogue_list(Some(5), Some(0)).await {
        Ok(response) => {
            let message = format!(
                "✅ Hydra API connection successful! Found {} games (total: {})",
                response.edges.len(),
                response.count
            );
            println!("{}", message);
            Ok(message)
        }
        Err(e) => {
            let error_msg = format!("❌ Hydra API connection failed: {}", e);
            eprintln!("{}", error_msg);
            Err(error_msg)
        }
    }
}
