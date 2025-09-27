import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

/**
 * Custom hook for managing metadata resources
 * Handles fetching and caching of filter metadata from backend
 */
export const useMetadata = (language = 'en') => {
  const [metadata, setMetadata] = useState(null);
  const [filterMetadata, setFilterMetadata] = useState(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState(null);

  // Filter category colors (matches Hydra's design)
  const filterCategoryColors = {
    genres: 'hsl(262deg 50% 47%)',        // Purple
    tags: 'hsl(95deg 50% 20%)',          // Green
    developers: 'hsl(340deg 50% 46%)',   // Pink
    publishers: 'hsl(200deg 50% 30%)',   // Blue
  };

  // Fetch full metadata resources
  const fetchMetadataResources = useCallback(async () => {
    try {
      setIsLoading(true);
      setError(null);
      
      console.log('🗂️ Fetching metadata resources...');
      const resources = await invoke('get_metadata_resources');
      
      setMetadata(resources);
      console.log('✅ Metadata resources loaded');
      
      return resources;
    } catch (err) {
      console.error('❌ Error fetching metadata resources:', err);
      setError(err.message || 'Failed to fetch metadata resources');
      return null;
    }
  }, []);

  // Fetch filter metadata for UI
  const fetchFilterMetadata = useCallback(async (lang = language) => {
    try {
      setIsLoading(true);
      setError(null);
      
      console.log(`🎯 Fetching filter metadata (${lang})...`);
      const filterData = await invoke('get_filter_metadata', { language: lang });
      
      setFilterMetadata(filterData);
      console.log('✅ Filter metadata loaded');
      
      return filterData;
    } catch (err) {
      console.error('❌ Error fetching filter metadata:', err);
      setError(err.message || 'Failed to fetch filter metadata');
      return null;
    } finally {
      setIsLoading(false);
    }
  }, [language]);

  // Test metadata connection
  const testConnection = useCallback(async () => {
    try {
      console.log('🔍 Testing metadata connection...');
      const result = await invoke('test_metadata_connection');
      console.log('✅ Connection test result:', result);
      return result;
    } catch (err) {
      console.error('❌ Connection test failed:', err);
      throw err;
    }
  }, []);

  // Process filter items to include checked state
  const processFilterItems = useCallback((items, selectedValues = []) => {
    if (!items) return [];
    
    return items.map(item => ({
      ...item,
      checked: selectedValues.includes(item.value)
    }));
  }, []);

  // Get filter sections for UI
  const getFilterSections = useCallback((selectedFilters = {}) => {
    if (!filterMetadata) return [];

    return [
      {
        title: 'Genres',
        key: 'genres',
        color: filterCategoryColors.genres,
        items: processFilterItems(filterMetadata.genres, selectedFilters.genres || [])
      },
      {
        title: 'Tags',
        key: 'tags',
        color: filterCategoryColors.tags,
        items: processFilterItems(filterMetadata.tags, selectedFilters.tags || [])
      },
      {
        title: 'Developers',
        key: 'developers',
        color: filterCategoryColors.developers,
        items: processFilterItems(filterMetadata.developers, selectedFilters.developers || [])
      },
      {
        title: 'Publishers',
        key: 'publishers',
        color: filterCategoryColors.publishers,
        items: processFilterItems(filterMetadata.publishers, selectedFilters.publishers || [])
      }
    ].filter(section => section.items.length > 0);
  }, [filterMetadata, filterCategoryColors, processFilterItems]);

  // Get grouped active filters for display
  const getGroupedFilters = useCallback((selectedFilters = {}) => {
    if (!filterMetadata) return [];

    const grouped = [];

    // Process each filter category
    Object.entries(selectedFilters).forEach(([category, values]) => {
      if (!values || !Array.isArray(values) || values.length === 0) return;

      const categoryData = filterMetadata[category];
      if (!categoryData) return;

      values.forEach(value => {
        const item = categoryData.find(item => item.value === value);
        if (item) {
          grouped.push({
            label: item.label,
            value: item.value,
            category,
            orbColor: filterCategoryColors[category]
          });
        }
      });
    });

    return grouped;
  }, [filterMetadata, filterCategoryColors]);

  // Initialize metadata on mount
  useEffect(() => {
    fetchFilterMetadata();
  }, [fetchFilterMetadata]);

  return {
    // Data
    metadata,
    filterMetadata,
    isLoading,
    error,
    
    // Colors
    filterCategoryColors,
    
    // Methods
    fetchMetadataResources,
    fetchFilterMetadata,
    testConnection,
    processFilterItems,
    getFilterSections,
    getGroupedFilters,
    
    // Computed
    hasMetadata: !!filterMetadata,
    isReady: !isLoading && !!filterMetadata && !error
  };
};
