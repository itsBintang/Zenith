import React, { useState, useEffect, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FiRefreshCw, FiPlay, FiClock } from 'react-icons/fi';
import { LibraryGameSkeleton } from './SkeletonLoader';

function LibraryGrid({ 
  onGameSelect, 
  showHeader = true, 
  title = "Library", 
  showFilter = true,
  showRefresh = true,
  gridView = true,
  libraryState,
  onRefreshLibrary,
  onUpdateFilter
}) {
  // Use shared state if provided, otherwise fallback to local state
  const [localGames, setLocalGames] = useState([]);
  const [localIsLoading, setLocalIsLoading] = useState(true);
  const [localError, setLocalError] = useState('');
  const [localFilter, setLocalFilter] = useState('');
  const [lastFetchTime, setLastFetchTime] = useState(0);
  
  // Use shared state if available, otherwise use local state
  const games = libraryState ? libraryState.games : localGames;
  const isLoading = libraryState ? libraryState.isLoading : localIsLoading;
  const error = libraryState ? libraryState.error : localError;
  const filter = libraryState ? libraryState.filter : localFilter;

  const fetchLibraryGames = async () => {
    // Use shared refresh if available, otherwise use local fetch
    if (onRefreshLibrary) {
      await onRefreshLibrary();
      return;
    }
    
    // Fallback to local fetch logic
    const now = Date.now();
    if (now - lastFetchTime < 2000) {
      console.log('Skipping fetch - too soon after last request');
      return;
    }
    
    setLastFetchTime(now);
    setLocalIsLoading(true);
    setLocalError('');
    try {
      const libraryGames = await invoke('get_library_games');
      setLocalGames(libraryGames || []);
    } catch (err) {
      console.error('Failed to fetch library games:', err);
      setLocalError('Failed to load library. Is Steam installed?');
    } finally {
      setLocalIsLoading(false);
    }
  };

  useEffect(() => {
    // Only fetch if using local state (no shared state provided)
    if (!libraryState) {
      fetchLibraryGames();
    }
  }, [libraryState]);

  const filteredGames = useMemo(() => {
    if (!filter) {
      return games;
    }
    return games.filter(game =>
      game.name.toLowerCase().includes(filter.toLowerCase())
    );
  }, [games, filter]);

  const formatPlayTime = (minutes) => {
    if (!minutes || minutes === 0) return '0 minutes';
    if (minutes < 60) return `${minutes} minutes`;
    const hours = Math.floor(minutes / 60);
    const remainingMinutes = minutes % 60;
    if (remainingMinutes === 0) return `${hours} hours`;
    return `${hours}h ${remainingMinutes}m`;
  };

  return (
    <div className={`library-container ${gridView ? 'library-grid-view' : 'library-list-view'}`}>
      {showHeader && (
        <div className="library-header">
          <div className="library-header-left">
            <h2 className="library-title">{title}</h2>
            {games.length > 0 && (
              <span className="library-count">{filteredGames.length}</span>
            )}
          </div>
          {showRefresh && (
            <button 
              className={`library-refresh-btn ${isLoading ? 'loading' : ''}`} 
              onClick={fetchLibraryGames}
              disabled={isLoading}
            >
              <FiRefreshCw size={16} />
            </button>
          )}
        </div>
      )}
      
      {showFilter && (
        <div className="library-filter-container">
          <input
            type="text"
            placeholder="Filter library"
            className="library-filter-input"
              value={filter}
              onChange={(e) => {
                if (onUpdateFilter) {
                  onUpdateFilter(e.target.value);
                } else {
                  setLocalFilter(e.target.value);
                }
              }}
          />
        </div>
      )}

      {gridView ? (
        /* Grid View - Like Steam Library */
        <div className="library-games-grid">
          {isLoading && (
            <>
              {[...Array(8)].map((_, index) => (
                <div key={index} className="library-game-card skeleton">
                  <div className="game-card-image-skeleton"></div>
                  <div className="game-card-overlay">
                    <div className="game-card-playtime skeleton-text"></div>
                    <div className="game-card-title skeleton-text"></div>
                  </div>
                </div>
              ))}
            </>
          )}
          {error && <div className="library-message error">{error}</div>}
          {!isLoading && !error && filteredGames.length === 0 && (
            <div className="library-message">
              {filter ? 'No games found' : 'Your library is empty'}
            </div>
          )}
          {!isLoading && filteredGames.map(game => (
            <div 
              key={game.app_id} 
              className="library-game-card" 
              onClick={() => onGameSelect && onGameSelect(game.app_id)}
            >
              <div className="game-card-image">
                <img src={game.header_image} alt={game.name} />
                <div className="game-card-overlay">
                  <div className="game-card-playtime">
                    <FiClock size={12} />
                    <span>{formatPlayTime(game.playtime_forever || 0)}</span>
                  </div>
                  <div className="game-card-play-button">
                    <FiPlay size={16} />
                  </div>
                </div>
              </div>
              <div className="game-card-info">
                <h3 className="game-card-title">{game.name}</h3>
              </div>
            </div>
          ))}
        </div>
      ) : (
        /* List View - Like Sidebar */
        <div className="library-games-list">
          {isLoading && (
            <>
              {[...Array(5)].map((_, index) => (
                <LibraryGameSkeleton key={index} />
              ))}
            </>
          )}
          {error && <div className="library-message error">{error}</div>}
          {!isLoading && !error && filteredGames.length === 0 && (
            <div className="library-message">
              {filter ? 'No games found' : 'Your library is empty'}
            </div>
          )}
          {!isLoading && filteredGames.map(game => (
            <button key={game.app_id} className="library-game-item" onClick={() => onGameSelect && onGameSelect(game.app_id)}>
              <img src={game.header_image} alt={game.name} className="game-item-icon" />
              <span className="game-item-name">{game.name}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

export default LibraryGrid;
