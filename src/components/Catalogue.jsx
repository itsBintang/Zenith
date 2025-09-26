import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useNavigate, useOutletContext } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import './Catalogue.css';

const Catalogue = () => {
  const navigate = useNavigate();
  const { globalSearchQuery } = useOutletContext();
  const [games, setGames] = useState([]);
  const [isLoading, setIsLoading] = useState(true);
  const [currentPage, setCurrentPage] = useState(1);
  const [totalPages, setTotalPages] = useState(0);
  const [totalGames, setTotalGames] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [error, setError] = useState(null);
  const [currentSearchQuery, setCurrentSearchQuery] = useState('');

  // Fetch games from SteamUI API
  const fetchGames = useCallback(async (page, searchQuery = '') => {
    setIsLoading(true);
    setError(null);
    
    try {
      let result;
      if (searchQuery && searchQuery.trim() !== '') {
        result = await invoke('search_steamui_games', { 
          query: searchQuery.trim(), 
          page 
        });
      } else {
        result = await invoke('fetch_steamui_games', { page });
      }
      
      setGames(result.games);
      setTotalPages(result.total_pages);
      setTotalGames(result.total_games);
      setHasMore(result.has_more);
      
    } catch (error) {
      console.error('Failed to fetch games:', error);
      setError(`Failed to load games: ${error}`);
      setGames([]);
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Effect for initial load and page changes
  useEffect(() => {
    fetchGames(currentPage, currentSearchQuery);
  }, [currentPage, currentSearchQuery, fetchGames]);

  // Function to extract App ID from Steam URL or detect pure AppID
  const extractAppIdFromUrl = (input) => {
    if (!input) return null;
    
    // Check if input is already a pure App ID (numeric)
    if (/^\d+$/.test(input.trim())) {
      return input.trim();
    }
    
    // Steam URL patterns:
    // https://store.steampowered.com/app/2622380/ELDEN_RING_NIGHTREIGN/
    // https://steamdb.info/app/578080/charts/
    // steam://store/851850
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

  // Handle search on Enter key press
  const handleSearchSubmit = useCallback(() => {
    if (globalSearchQuery !== currentSearchQuery) {
      // Check if the search query is a Steam URL or App ID
      const extractedAppId = extractAppIdFromUrl(globalSearchQuery);
      
      if (extractedAppId) {
        // Navigate directly to game detail page if it's a Steam URL or App ID
        navigate(`/game/${extractedAppId}`);
        return;
      }

      // Otherwise, perform normal search
      setCurrentSearchQuery(globalSearchQuery);
      setCurrentPage(1); // Reset to first page for new search
    }
  }, [globalSearchQuery, currentSearchQuery, navigate]);

  // Listen for Enter key in search input
  useEffect(() => {
    const handleKeyDown = (event) => {
      if (event.key === 'Enter' && document.activeElement?.type === 'text') {
        handleSearchSubmit();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [handleSearchSubmit]);

  // Listen for global search trigger from Header
  useEffect(() => {
    const handleTriggerSearch = (event) => {
      const { query } = event.detail;
      
      if (query && query.trim() !== '') {
        // Check if it's AppID (shouldn't happen here, but just in case)
        const extractedAppId = extractAppIdFromUrl(query);
        if (extractedAppId) {
          navigate(`/game/${extractedAppId}`);
          return;
        }
        
        // Perform name search
        setCurrentSearchQuery(query.trim());
        setCurrentPage(1);
      }
    };

    window.addEventListener('triggerSearch', handleTriggerSearch);
    
    return () => {
      window.removeEventListener('triggerSearch', handleTriggerSearch);
    };
  }, [navigate]);

  const handleGameCardClick = (appId) => {
    navigate(`/game/${appId}`);
  };

  const handlePageChange = (pageNumber) => {
    if (pageNumber >= 1 && pageNumber !== currentPage) {
      // For next page, check if we have more data
      if (pageNumber > currentPage && pageNumber > totalPages && !hasMore) {
        return; // Don't allow going to next page if no more data
      }
      setCurrentPage(pageNumber);
    }
  };

  const getPageNumbers = () => {
    const pageNumbers = [];
    const maxVisiblePages = 5;
    
    // Show current page and a few around it, plus next if has_more
    const startPage = Math.max(1, currentPage - 2);
    const endPage = hasMore ? currentPage + 2 : Math.max(currentPage, totalPages);
    
    // Always show page 1
    if (startPage > 1) {
      pageNumbers.push(1);
      if (startPage > 2) {
        pageNumbers.push('...');
      }
    }
    
    // Show pages around current
    for (let i = startPage; i <= endPage; i++) {
      pageNumbers.push(i);
    }
    
    // Show more pages indicator if has_more
    if (hasMore && endPage === currentPage + 2) {
      pageNumbers.push('...');
    }
    
    return pageNumbers;
  };

  return (
    <div className="ui-page">
      <div className="catalogue-body">
        <div className="game-list full-width">
          {isLoading ? (
            <>
              <div className="skeleton-info">
                <div className="skeleton-info-text"></div>
                 {globalSearchQuery && globalSearchQuery !== currentSearchQuery && (
                   <div className="search-feedback">
                     {extractAppIdFromUrl(globalSearchQuery) ? (
                       <span>Press Enter to open App ID: {extractAppIdFromUrl(globalSearchQuery)}</span>
                     ) : (
                       <span>Press Enter to search for "{globalSearchQuery}"</span>
                     )}
                   </div>
                 )}
              </div>
              <div className="skeleton-container">
                {Array.from({ length: 12 }, (_, index) => (
                  <div key={index} className="skeleton-game-card">
                    <div className="skeleton-image"></div>
                     <div className="skeleton-content">
                       <div className="skeleton-title"></div>
                     </div>
                  </div>
                ))}
              </div>
            </>
          ) : error ? (
            <div className="error-container">
              <h3>Error Loading Games</h3>
              <p>{error}</p>
              <button onClick={() => fetchGames(currentPage, currentSearchQuery)}>
                Try Again
              </button>
            </div>
          ) : games.length > 0 ? (
            <>
              {games.map((game, index) => (
                <div 
                  key={`${game.app_id}-${index}`} 
                  className="game-card"
                  onClick={() => handleGameCardClick(game.app_id)}
                >
                  <div className="game-image-container">
                    <img 
                      src={game.header_image} 
                      alt={game.name} 
                      className="game-image" 
                      loading="lazy" 
                      width="231" 
                      height="87"
                      onError={(e) => {
                        // Fallback to header.jpg if library_hero.jpg fails
                        if (e.target.src.includes('/library_hero.jpg')) {
                          e.target.src = `https://cdn.akamai.steamstatic.com/steam/apps/${game.app_id}/header.jpg`;
                        }
                      }}
                    />
                    {game.is_free && (
                      <div className="free-badge">
                        FREE
                      </div>
                    )}
                  </div>
                   <div className="game-info">
                     <h3>{game.name}</h3>
                     {game.is_free && (
                       <div className="game-meta">
                         <span className="free-tag">Free to Play</span>
                       </div>
                     )}
                   </div>
                </div>
              ))}
              
              {/* Pagination */}
              {(currentPage > 1 || hasMore) && (
                <div className="pagination">
                  <button 
                    className="pagination-btn"
                    onClick={() => handlePageChange(currentPage - 1)}
                    disabled={currentPage === 1}
                  >
                    Previous
                  </button>
                  
                  {getPageNumbers().map((pageNum, index) => (
                    <button
                      key={index}
                      className={`pagination-btn ${pageNum === currentPage ? 'active' : ''} ${pageNum === '...' ? 'dots' : ''}`}
                      onClick={() => typeof pageNum === 'number' && handlePageChange(pageNum)}
                      disabled={pageNum === '...'}
                    >
                      {pageNum}
                    </button>
                  ))}
                  
                  <button 
                    className="pagination-btn"
                    onClick={() => handlePageChange(currentPage + 1)}
                    disabled={!hasMore}
                  >
                    Next
                  </button>
                </div>
              )}
            </>
          ) : (
            <div className="no-results">
              <div className="placeholder-content">
                <h2>No Games Found</h2>
                <p>Try searching for something else</p>
                 {globalSearchQuery && (
                   <p>
                     {extractAppIdFromUrl(globalSearchQuery) ? (
                       <>Press Enter to open App ID: {extractAppIdFromUrl(globalSearchQuery)}</>
                     ) : (
                       <>Press Enter to search for: "{globalSearchQuery}"</>
                     )}
                   </p>
                 )}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default Catalogue;