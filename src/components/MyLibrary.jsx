import React, { useState, useEffect, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FiRefreshCw, FiPlay } from 'react-icons/fi';
import { LibraryGameSkeleton } from './SkeletonLoader';
import '../styles/MyLibrary.css';

function MyLibrary({ onGameSelect }) {
  const [games, setGames] = useState([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState('');
  const [filter, setFilter] = useState('');
  const [lastFetchTime, setLastFetchTime] = useState(0);

  const fetchLibraryGames = async () => {
    // Prevent rapid successive calls (minimum 2 seconds between fetches)
    const now = Date.now();
    if (now - lastFetchTime < 2000) {
      console.log('Skipping fetch - too soon after last request');
      return;
    }
    
    setLastFetchTime(now);
    setIsLoading(true);
    setError('');
    try {
      const libraryGames = await invoke('get_library_games');
      setGames(libraryGames || []);
    } catch (err) {
      console.error('Failed to fetch library games:', err);
      setError('Failed to load library. Is Steam installed?');
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchLibraryGames();
  }, []);

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
          onChange={(e) => setFilter(e.target.value)}
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
