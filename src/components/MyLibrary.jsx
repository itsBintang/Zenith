import React, { useState, useEffect, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FiRefreshCw, FiPlay } from 'react-icons/fi';
import { LibraryGameSkeleton } from './SkeletonLoader';
import '../styles/MyLibrary.css';

function MyLibrary({ onGameSelect, libraryState, onRefreshLibrary, onUpdateFilter }) {
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

  return (
    <div className="my-library-container">
      <div className="library-header">
        <h2 className="library-title">MY LIBRARY</h2>
        <button 
          className={`library-refresh-btn ${isLoading ? 'loading' : ''}`} 
          onClick={fetchLibraryGames}
          disabled={isLoading}
        >
          <FiRefreshCw size={14} />
        </button>
      </div>
      
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
          <button key={game.app_id} className="library-game-item" onClick={() => onGameSelect(game.app_id)}>
            <img src={game.header_image} alt={game.name} className="game-item-icon" />
            <span className="game-item-name">{game.name}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

export default MyLibrary;
