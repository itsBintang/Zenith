/// TTL Configuration for different types of game data based on their dynamism
/// Values are in seconds for precise control

/// TTL Constants based on data dynamism analysis
pub struct TtlConfig;

impl TtlConfig {
    // ========== DYNAMIC DATA (TTL Sedang) ==========
    /// DLC List - can be updated with new releases but not that frequently
    pub const DLC_LIST: i64 = 30 * 24 * 3600; // 30 days (increased from 3 days)
    
    // ========== STATIC DATA (TTL Panjang) ==========
    /// Screenshots - rarely change unless major update
    pub const SCREENSHOTS: i64 = 90 * 24 * 3600; // 90 days (increased from 30 days)
    
    /// Detailed descriptions - very stable content
    pub const DETAILED_DESCRIPTION: i64 = 90 * 24 * 3600; // 90 days (increased from 30 days)
    
    /// System requirements - only change with major updates
    pub const SYSTEM_REQUIREMENTS: i64 = 180 * 24 * 3600; // 180 days (increased from 60 days)
    
    /// Publisher - almost never changes
    pub const PUBLISHER: i64 = 365 * 24 * 3600; // 1 year (permanent-like)
    
    /// Release date - never changes after release
    pub const RELEASE_DATE: i64 = 365 * 24 * 3600; // 1 year (permanent-like)
    
    // ========== SEMI-STATIC DATA (TTL Sedang-Panjang) ==========
    /// Game name - can change but rarely
    pub const GAME_NAME: i64 = 60 * 24 * 3600; // 60 days (increased from 30 days)
    
    /// Header image - updated occasionally
    pub const HEADER_IMAGE: i64 = 60 * 24 * 3600; // 60 days (increased from 21 days)
    
    /// Banner image - updated occasionally  
    pub const BANNER_IMAGE: i64 = 60 * 24 * 3600; // 60 days (increased from 21 days)
    
    /// Trailer - can be updated with major releases
    pub const TRAILER: i64 = 60 * 24 * 3600; // 60 days (increased from 21 days)
    
    /// DRM notice - changes rarely but can happen
    pub const DRM_NOTICE: i64 = 90 * 24 * 3600; // 90 days (increased from 60 days)
    
    // ========== DEFAULT FALLBACK ==========
    /// Default TTL for unknown/mixed data
    pub const DEFAULT: i64 = 30 * 24 * 3600; // 30 days (increased from 7 days)
}

/// TTL categories for easier management
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TtlCategory {
    /// High-frequency updates (hours to days)
    Dynamic,
    /// Medium-frequency updates (weeks)
    SemiStatic, 
    /// Low-frequency updates (months to years)
    Static,
}

impl TtlCategory {
    /// Get appropriate TTL for the category
    pub fn default_ttl(&self) -> i64 {
        match self {
            TtlCategory::Dynamic => 30 * 24 * 3600,      // 30 days (increased from 3 days)
            TtlCategory::SemiStatic => 60 * 24 * 3600,   // 60 days (increased from 21 days)  
            TtlCategory::Static => 90 * 24 * 3600,       // 90 days (increased from 60 days)
        }
    }
    
    /// Get category name for logging
    pub fn name(&self) -> &'static str {
        match self {
            TtlCategory::Dynamic => "Dynamic",
            TtlCategory::SemiStatic => "Semi-Static",
            TtlCategory::Static => "Static",
        }
    }
}

/// Field-specific TTL mapping
pub struct FieldTtl;

impl FieldTtl {
    /// Get TTL for specific GameDetail field
    pub fn get_field_ttl(field_name: &str) -> i64 {
        match field_name {
            // Dynamic data
            "dlc" => TtlConfig::DLC_LIST,
            
            // Static data  
            "screenshots" => TtlConfig::SCREENSHOTS,
            "detailed_description" => TtlConfig::DETAILED_DESCRIPTION,
            "sysreq_min" | "sysreq_rec" | "pc_requirements" => TtlConfig::SYSTEM_REQUIREMENTS,
            "publisher" => TtlConfig::PUBLISHER,
            "release_date" => TtlConfig::RELEASE_DATE,
            
            // Semi-static data
            "name" => TtlConfig::GAME_NAME,
            "header_image" => TtlConfig::HEADER_IMAGE,
            "banner_image" => TtlConfig::BANNER_IMAGE,
            "trailer" => TtlConfig::TRAILER,
            "drm_notice" => TtlConfig::DRM_NOTICE,
            
            // Default fallback
            _ => TtlConfig::DEFAULT,
        }
    }
    
    /// Get category for specific field
    pub fn get_field_category(field_name: &str) -> TtlCategory {
        match field_name {
            "dlc" => TtlCategory::Dynamic,
            
            "screenshots" | "detailed_description" | "sysreq_min" | "sysreq_rec" | 
            "pc_requirements" | "publisher" | "release_date" => TtlCategory::Static,
            
            "name" | "header_image" | "banner_image" | "trailer" | "drm_notice" => 
                TtlCategory::SemiStatic,
            
            _ => TtlCategory::SemiStatic, // Safe default
        }
    }
    
    /// Get all field names grouped by category
    pub fn get_fields_by_category(category: TtlCategory) -> Vec<&'static str> {
        match category {
            TtlCategory::Dynamic => vec!["dlc"],
            
            TtlCategory::Static => vec![
                "screenshots", "detailed_description", "sysreq_min", "sysreq_rec", 
                "pc_requirements", "publisher", "release_date"
            ],
            
            TtlCategory::SemiStatic => vec![
                "name", "header_image", "banner_image", "trailer", "drm_notice"
            ],
        }
    }
}

/// Smart TTL calculator for mixed refresh scenarios
pub struct SmartTtl;

impl SmartTtl {
    /// Calculate optimal TTL for full game detail refresh
    /// Uses the dynamic data TTL as baseline for critical data
    pub fn calculate_full_refresh_ttl() -> i64 {
        TtlConfig::DLC_LIST // Use dynamic data TTL as baseline (30 days)
    }
    
    /// Calculate TTL for partial refresh of specific fields
    pub fn calculate_partial_refresh_ttl(fields: &[&str]) -> i64 {
        fields.iter()
            .map(|field| FieldTtl::get_field_ttl(field))
            .min()
            .unwrap_or(TtlConfig::DEFAULT)
    }
    
    /// Determine which fields need refresh based on individual TTL
    pub fn get_expired_fields(last_updated: i64, current_time: i64) -> Vec<&'static str> {
        let mut expired_fields = Vec::new();
        
        // Check each field category
        for category in [TtlCategory::Dynamic, TtlCategory::SemiStatic, TtlCategory::Static] {
            let fields = FieldTtl::get_fields_by_category(category);
            let ttl = category.default_ttl();
            
            if current_time - last_updated > ttl {
                expired_fields.extend(fields);
            }
        }
        
        expired_fields
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ttl_hierarchy() {
        // Dynamic should be shortest
        assert!(TtlConfig::DLC_LIST < TtlConfig::GAME_NAME);
        
        // Semi-static should be medium
        assert!(TtlConfig::GAME_NAME < TtlConfig::SCREENSHOTS);
        
        // Static should be longest
        assert!(TtlConfig::SCREENSHOTS < TtlConfig::PUBLISHER);
    }
    
    #[test]
    fn test_field_categorization() {
        assert_eq!(FieldTtl::get_field_category("dlc"), TtlCategory::Dynamic);
        assert_eq!(FieldTtl::get_field_category("name"), TtlCategory::SemiStatic);
        assert_eq!(FieldTtl::get_field_category("screenshots"), TtlCategory::Static);
    }
    
    #[test]
    fn test_smart_ttl_calculation() {
        let fields = vec!["dlc", "screenshots", "name"];
        let ttl = SmartTtl::calculate_partial_refresh_ttl(&fields);
        // Should use the shortest TTL (dlc = 30 days)
        assert_eq!(ttl, TtlConfig::DLC_LIST);
    }
}
