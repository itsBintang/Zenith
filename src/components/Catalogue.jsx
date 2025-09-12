import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SearchResultsSkeleton } from "./SkeletonLoader";

function Catalogue({ onGameSelect, catalogueState, setCatalogueState }) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  
  const { query, results, hasSearched } = catalogueState;

  const onSearch = async () => {
    if (!query.trim()) return;
    setLoading(true);
    setError("");
    setCatalogueState(prev => ({ ...prev, hasSearched: true }));
    try {
      const res = await invoke("search_games", { query: query.trim() });
      setCatalogueState(prev => ({ ...prev, results: res || [] }));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };


  return (
    <div className="ui-page">
      <div className="ui-subheader">
        <div className="ui-header__title">Catalogue</div>
        <div className="ui-header__actions">
          <div className="ui-input ui-input--search">
            <input
              placeholder="Search by AppID, name, or Steam URL"
              value={query}
              onChange={(e) => {
                setCatalogueState(prev => ({ ...prev, query: e.target.value }));
                // Reset search state when query is cleared
                if (e.target.value.trim() === "") {
                  setCatalogueState(prev => ({ 
                    ...prev, 
                    hasSearched: false, 
                    results: [] 
                  }));
                }
              }}
              onKeyDown={(e) => e.key === "Enter" && onSearch()}
            />
          </div>
          <button className="ui-btn" onClick={onSearch} disabled={loading}>
            {loading ? "Searching..." : "Search"}
          </button>
        </div>
      </div>

      <div className="ui-content">
        {error && <div className="status-message">{error}</div>}
        
        {loading && <SearchResultsSkeleton />}
        
        {!loading && !error && (
          <>
            {results.length > 0 && (
              <div className="search-results-info">
                Found {results.length} game{results.length !== 1 ? 's' : ''}
              </div>
            )}
            
            {results.length === 0 && hasSearched && !loading && (
              <div className="status-message">
                No games found for "{query}". Try different search terms or check AppID.
              </div>
            )}
            
            <div className="ui-grid">
              {results.map((g) => (
                <div className="ui-card ui-card--hero" key={g.app_id} onClick={() => onGameSelect && onGameSelect(g.app_id)} style={{ cursor: 'pointer' }}>
                  <div 
                    className="ui-card__hero-image"
                    style={{
                      backgroundImage: `url(${g.header_image})`,
                      backgroundSize: "cover",
                      backgroundPosition: "center"
                    }}
                  />
                  <div className="ui-card__hero-overlay">
                    <h3 className="ui-card__hero-title">{g.name}</h3>
                    <div className="ui-card__hero-appid">AppID: {g.app_id}</div>
                  </div>
                </div>
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

export default Catalogue;


