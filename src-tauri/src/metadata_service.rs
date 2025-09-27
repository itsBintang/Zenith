use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::command;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetadataResources {
    pub genres: HashMap<String, Vec<String>>,
    pub tags: HashMap<String, HashMap<String, u32>>,
    pub developers: Vec<String>,
    pub publishers: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FilterMetadata {
    pub genres: Vec<FilterItem>,
    pub tags: Vec<FilterItem>,
    pub developers: Vec<FilterItem>,
    pub publishers: Vec<FilterItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FilterItem {
    pub label: String,
    pub value: String,
    pub count: Option<u32>,
}

pub struct MetadataService {
    client: Client,
    base_url: String,
}

impl MetadataService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://assets.hydralauncher.gg".to_string(),
        }
    }

    /// Fetch Steam genres from external resources
    pub async fn fetch_steam_genres(&self) -> Result<HashMap<String, Vec<String>>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/steam-genres.json", self.base_url);
        let response = self.client.get(&url).send().await?;
        let genres: HashMap<String, Vec<String>> = response.json().await?;
        
        println!("✅ Fetched {} language genres", genres.keys().len());
        Ok(genres)
    }

    /// Fetch Steam user tags from external resources
    pub async fn fetch_steam_tags(&self) -> Result<HashMap<String, HashMap<String, u32>>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/steam-user-tags.json", self.base_url);
        let response = self.client.get(&url).send().await?;
        let tags: HashMap<String, HashMap<String, u32>> = response.json().await?;
        
        println!("✅ Fetched {} language tags", tags.keys().len());
        Ok(tags)
    }

    /// Fetch Steam developers from external resources
    pub async fn fetch_steam_developers(&self) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/steam-developers.json", self.base_url);
        let response = self.client.get(&url).send().await?;
        let developers: Vec<String> = response.json().await?;
        
        println!("✅ Fetched {} developers", developers.len());
        Ok(developers)
    }

    /// Fetch Steam publishers from external resources
    pub async fn fetch_steam_publishers(&self) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/steam-publishers.json", self.base_url);
        let response = self.client.get(&url).send().await?;
        let publishers: Vec<String> = response.json().await?;
        
        println!("✅ Fetched {} publishers", publishers.len());
        Ok(publishers)
    }

    /// Fetch all metadata resources
    pub async fn fetch_all_metadata(&self) -> Result<MetadataResources, Box<dyn std::error::Error + Send + Sync>> {
        println!("🗂️ Fetching all metadata resources...");
        
        let (genres, tags, developers, publishers) = tokio::try_join!(
            self.fetch_steam_genres(),
            self.fetch_steam_tags(), 
            self.fetch_steam_developers(),
            self.fetch_steam_publishers()
        )?;

        let metadata = MetadataResources {
            genres,
            tags,
            developers,
            publishers,
        };

        println!("✅ All metadata resources fetched successfully");
        Ok(metadata)
    }

    /// Generate filter metadata for UI (popular/top items)
    pub fn generate_filter_metadata(&self, metadata: &MetadataResources, language: &str) -> FilterMetadata {
        let default_lang = "en";
        let lang = if metadata.genres.contains_key(language) { language } else { default_lang };

        // Process genres
        let genres = metadata.genres.get(lang)
            .unwrap_or(&Vec::new())
            .iter()
            .take(20) // Top 20 genres
            .map(|genre| FilterItem {
                label: genre.clone(),
                value: genre.clone(),
                count: None,
            })
            .collect();

        // Process tags
        let tags = metadata.tags.get(lang)
            .unwrap_or(&HashMap::new())
            .iter()
            .take(30) // Top 30 tags
            .map(|(name, id)| FilterItem {
                label: name.clone(),
                value: id.to_string(),
                count: None,
            })
            .collect();

        // Process developers (top 50)
        let developers = metadata.developers
            .iter()
            .take(50)
            .map(|dev| FilterItem {
                label: dev.clone(),
                value: dev.clone(),
                count: None,
            })
            .collect();

        // Process publishers (top 50)
        let publishers = metadata.publishers
            .iter()
            .take(50)
            .map(|pub_name| FilterItem {
                label: pub_name.clone(),
                value: pub_name.clone(),
                count: None,
            })
            .collect();

        FilterMetadata {
            genres,
            tags,
            developers,
            publishers,
        }
    }
}

// Global metadata service instance
lazy_static::lazy_static! {
    static ref METADATA_SERVICE: MetadataService = MetadataService::new();
}

/// Tauri command to fetch all metadata
#[command]
pub async fn get_metadata_resources() -> Result<MetadataResources, String> {
    METADATA_SERVICE
        .fetch_all_metadata()
        .await
        .map_err(|e| {
            eprintln!("❌ Error fetching metadata: {}", e);
            format!("Failed to fetch metadata: {}", e)
        })
}

/// Tauri command to get filter metadata for UI
#[command]
pub async fn get_filter_metadata(language: Option<String>) -> Result<FilterMetadata, String> {
    let lang = language.as_deref().unwrap_or("en");
    
    let metadata = METADATA_SERVICE
        .fetch_all_metadata()
        .await
        .map_err(|e| {
            eprintln!("❌ Error fetching metadata: {}", e);
            format!("Failed to fetch metadata: {}", e)
        })?;

    let filter_metadata = METADATA_SERVICE.generate_filter_metadata(&metadata, lang);
    
    println!("✅ Generated filter metadata for language: {}", lang);
    Ok(filter_metadata)
}

/// Tauri command to test metadata connection
#[command]
pub async fn test_metadata_connection() -> Result<String, String> {
    println!("🔍 Testing metadata connection...");
    
    match METADATA_SERVICE.fetch_steam_genres().await {
        Ok(genres) => {
            let genre_count = genres.values().map(|v| v.len()).sum::<usize>();
            Ok(format!("✅ Metadata connection successful! Found {} genres across {} languages", 
                      genre_count, genres.keys().len()))
        }
        Err(e) => {
            eprintln!("❌ Metadata connection failed: {}", e);
            Err(format!("Metadata connection failed: {}", e))
        }
    }
}
