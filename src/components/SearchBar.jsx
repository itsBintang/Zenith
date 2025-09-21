import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faSearch, faSpinner } from '@fortawesome/free-solid-svg-icons';
import './SearchBar.css';

const SearchBar = ({ 
  onResultSelect, 
  useCatalogue = false, 
  placeholder = "Search games, App ID, or Steam URL",
  className = ""
}) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState([]);
  const [isSearching, setIsSearching] = useState(false);
  const [showResults, setShowResults] = useState(false);

  // Function to extract App ID from Steam URL
  const extractAppIdFromUrl = (input) => {
    if (!input) return null;
    
    // Check if input is already a pure App ID (numeric)
    if (/^\d+$/.test(input.trim())) {
      return input.trim();
    }
    
    // Steam URL patterns
    const steamUrlPatterns = [
      /store\.steampowered\.com\/app\/(\d+)/,
      /steamdb\.info\/app\/(\d+)/,
      /steam:\/\/store\/(\d+)/
    ];
    
    for (const pattern of steamUrlPatterns) {
      const match = input.match(pattern);
      if (match) {
        return match[1];
      }
    }
    
    return null;
  };

  const handleSearch = async (query) => {
    if (!query || query.trim().length < 2) {
      setSearchResults([]);
      setShowResults(false);
      return;
    }

    setIsSearching(true);
    
    try {
      // Check if it's an App ID or URL first
      const extractedAppId = extractAppIdFromUrl(query);
      
      if (extractedAppId) {
        // If it's an App ID or URL, select it directly
        if (onResultSelect) {
          onResultSelect(extractedAppId);
        }
        setSearchQuery('');
        setShowResults(false);
        return;
      }

      // Otherwise, search for games
      const results = await invoke('search_games_hybrid', {
        query: query.trim(),
        useCatalogue,
        limit: 10
      });

      setSearchResults(results || []);
      setShowResults(true);
    } catch (error) {
      console.error('Search failed:', error);
      setSearchResults([]);
      setShowResults(false);
    } finally {
      setIsSearching(false);
    }
  };

  const handleInputChange = (e) => {
    const value = e.target.value;
    setSearchQuery(value);
  };

  const handleKeyPress = (e) => {
    if (e.key === 'Enter') {
      handleSearch(searchQuery);
    }
  };

  const handleResultClick = (result) => {
    if (onResultSelect) {
      onResultSelect(result.app_id);
    }
    setSearchQuery('');
    setSearchResults([]);
    setShowResults(false);
  };

  const handleBlur = () => {
    // Delay hiding results to allow clicking
    setTimeout(() => {
      setShowResults(false);
    }, 200);
  };

  // Debounced search
  useEffect(() => {
    const timeoutId = setTimeout(() => {
      if (searchQuery && searchQuery.trim().length >= 2) {
        handleSearch(searchQuery);
      }
    }, 300);

    return () => clearTimeout(timeoutId);
  }, [searchQuery]);

  return (
    <div className={`search-bar-container ${className}`}>
      <div className="search-bar-input-wrapper">
        <FontAwesomeIcon icon={faSearch} className="search-bar-icon" />
        <input
          type="text"
          placeholder={placeholder}
          value={searchQuery}
          onChange={handleInputChange}
          onKeyPress={handleKeyPress}
          onBlur={handleBlur}
          className="search-bar-input"
        />
        {isSearching && (
          <FontAwesomeIcon icon={faSpinner} className="search-bar-spinner spinning" />
        )}
      </div>
      
      {showResults && searchResults.length > 0 && (
        <div className="search-results-dropdown">
          {searchResults.map((result, index) => (
            <div
              key={`${result.app_id}-${index}`}
              className="search-result-item"
              onClick={() => handleResultClick(result)}
            >
              <img 
                src={result.header_image} 
                alt={result.name}
                className="search-result-image"
                onError={(e) => {
                  e.target.style.display = 'none';
                }}
              />
              <div className="search-result-info">
                <div className="search-result-name">{result.name}</div>
                <div className="search-result-meta">
                  <span className="search-result-appid">App ID: {result.app_id}</span>
                  <span className={`search-result-source ${result.source}`}>
                    {result.source === 'catalogue' ? '📊 Catalogue' : '🔍 Steam API'}
                  </span>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default SearchBar;
