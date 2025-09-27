use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Hydra API client for fetching game catalogue data
pub struct HydraApi {
    client: Client,
    base_url: String,
}

// Data structures matching Hydra API responses
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HydraGame {
    pub id: String,
    #[serde(rename = "objectId")]
    pub object_id: String,
    pub title: String,
    pub shop: String,
    pub genres: Vec<String>,
    #[serde(rename = "libraryImageUrl")]
    pub library_image_url: Option<String>,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct CatalogueResponse {
    pub edges: Vec<HydraGame>,
    pub count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CatalogueSearchPayload {
    pub title: String,
    pub take: u32,
    pub skip: u32,
    #[serde(rename = "downloadSourceFingerprints")]
    pub download_source_fingerprints: Vec<String>,
    pub publishers: Vec<String>,
    pub genres: Vec<String>,
    pub developers: Vec<String>,
}

impl Default for CatalogueSearchPayload {
    fn default() -> Self {
        Self {
            title: String::new(),
            take: 20, // Default page size like Hydra
            skip: 0,
            download_source_fingerprints: Vec::new(),
            publishers: Vec::new(),
            genres: Vec::new(),
            developers: Vec::new(),
        }
    }
}

impl HydraApi {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("Zenith-Launcher/1.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: "https://hydra-api-us-east-1.losbroxas.org".to_string(),
        }
    }

    /// Fetch catalogue list (all games or filtered)
    /// Based on your hydra-test/endpoints/catalogue-list.js implementation
    pub async fn get_catalogue_list(
        &self,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<CatalogueResponse> {
        let mut payload = CatalogueSearchPayload::default();
        
        // Ensure minimum 5 items as per API requirement
        payload.take = std::cmp::max(limit.unwrap_or(20), 5);
        payload.skip = offset.unwrap_or(0);

        println!("📚 Fetching catalogue list (limit: {}, offset: {})", payload.take, payload.skip);

        let response = self
            .client
            .post(&format!("{}/catalogue/search", self.base_url))
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Hydra API returned error status: {}",
                response.status()
            ));
        }

        let catalogue_response: CatalogueResponse = response.json().await?;

        println!(
            "✅ Catalogue games: {} games found (total: {})",
            catalogue_response.edges.len(),
            catalogue_response.count
        );

        // Debug: Log first game's raw data from API
        if let Some(first_game) = catalogue_response.edges.first() {
            println!(
                "🔍 First game from API: title='{}', objectId='{}', libraryImageUrl='{:?}'",
                first_game.title, first_game.object_id, first_game.library_image_url
            );
        }

        Ok(catalogue_response)
    }

    /// Get paginated catalogue (like Hydra's pagination system)
    pub async fn get_paginated_catalogue(
        &self,
        page: u32,
        items_per_page: u32,
    ) -> Result<PaginatedCatalogueResponse> {
        let page = std::cmp::max(page, 1); // Ensure page is at least 1
        let items_per_page = std::cmp::max(items_per_page, 5); // API minimum
        let offset = (page - 1) * items_per_page;

        println!(
            "📄 Getting catalogue page {} ({} items per page)...",
            page, items_per_page
        );

        let results = self.get_catalogue_list(Some(items_per_page), Some(offset)).await?;

        let total_pages = if results.count > 0 {
            (results.count as f64 / items_per_page as f64).ceil() as u32
        } else {
            1
        };

        println!(
            "📊 Page {} of {} (Total games: {})",
            page, total_pages, results.count
        );

        Ok(PaginatedCatalogueResponse {
            games: results.edges.into_iter()
                .map(|hydra_game| crate::catalogue_commands::CatalogueGame::from(hydra_game))
                .collect(),
            pagination: PaginationInfo {
                current_page: page,
                total_pages,
                total_items: results.count,
                items_per_page,
                has_next_page: page < total_pages,
                has_prev_page: page > 1,
            },
        })
    }

    /// Search games with query and filters
    pub async fn search_games(
        &self,
        query: String,
        filters: Option<SearchFilters>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<CatalogueResponse> {
        let mut payload = CatalogueSearchPayload::default();
        
        payload.title = query;
        payload.take = std::cmp::max(limit.unwrap_or(20), 5);
        payload.skip = offset.unwrap_or(0);

        if let Some(filters) = filters {
            payload.genres = filters.genres;
            payload.developers = filters.developers;
            payload.publishers = filters.publishers;
            payload.download_source_fingerprints = filters.download_source_fingerprints;
        }

        println!("🔍 Searching games: \"{}\"", payload.title);

        let response = self
            .client
            .post(&format!("{}/catalogue/search", self.base_url))
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Hydra API search failed with status: {}",
                response.status()
            ));
        }

        let search_response: CatalogueResponse = response.json().await?;

        println!(
            "✅ Search results: {} games found (total: {})",
            search_response.edges.len(),
            search_response.count
        );

        Ok(search_response)
    }

    /// Search games with pagination (like get_paginated_catalogue but with search query)
    pub async fn search_games_paginated(
        &self,
        query: String,
        page: u32,
        items_per_page: u32,
    ) -> Result<PaginatedCatalogueResponse> {
        let page = std::cmp::max(page, 1); // Ensure page is at least 1
        let items_per_page = std::cmp::max(items_per_page, 5); // API minimum
        let offset = (page - 1) * items_per_page;

        let mut payload = CatalogueSearchPayload::default();
        payload.title = query.clone();
        payload.take = items_per_page;
        payload.skip = offset;

        println!("🔍 Searching games with query: '{}' (page: {})", query, page);

        let response = self
            .client
            .post(&format!("{}/catalogue/search", self.base_url))
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Hydra API search failed with status: {}",
                response.status()
            ));
        }

        let results: CatalogueResponse = response.json().await?;

        // Debug: Log first search result from API
        if let Some(first_game) = results.edges.first() {
            println!(
                "🔍 First search result from API: title='{}', objectId='{}', libraryImageUrl='{:?}'",
                first_game.title, first_game.object_id, first_game.library_image_url
            );
        }

        let total_pages = ((results.count as f64) / (items_per_page as f64)).ceil() as u32;

        println!(
            "📊 Search page {} of {} (Total games: {})",
            page, total_pages, results.count
        );

        Ok(PaginatedCatalogueResponse {
            games: results.edges.into_iter()
                .map(|hydra_game| crate::catalogue_commands::CatalogueGame::from(hydra_game))
                .collect(),
            pagination: PaginationInfo {
                current_page: page,
                total_pages,
                total_items: results.count,
                items_per_page,
                has_next_page: page < total_pages,
                has_prev_page: page > 1,
            },
        })
    }
}

// Supporting data structures
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedCatalogueResponse {
    pub games: Vec<crate::catalogue_commands::CatalogueGame>,
    pub pagination: PaginationInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationInfo {
    pub current_page: u32,
    pub total_pages: u32,
    pub total_items: u32,
    pub items_per_page: u32,
    pub has_next_page: bool,
    pub has_prev_page: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchFilters {
    pub genres: Vec<String>,
    pub developers: Vec<String>,
    pub publishers: Vec<String>,
    pub download_source_fingerprints: Vec<String>,
}

// Lazy static instance for reuse
use std::sync::LazyLock;
pub static HYDRA_API: LazyLock<HydraApi> = LazyLock::new(HydraApi::new);
