import React, { useState, useEffect, useCallback } from 'react';
import { useNavigate, useOutletContext } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import './Catalogue.css';

const Catalogue = () => {
  const navigate = useNavigate();
  const { globalSearchQuery } = useOutletContext();
  const [games, setGames] = useState([]);
  const [showApiResults, setShowApiResults] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  
  const [genres, setGenres] = useState([]);
  const [selectedGenres, setSelectedGenres] = useState([]);
  const [genreFilter, setGenreFilter] = useState('');
  
  const [currentPage, setCurrentPage] = useState(1);
  const [totalPages, setTotalPages] = useState(0);
  const gamesPerPage = 20;

  const fetchGames = useCallback(async (page, query, genreFilters = []) => {
    console.log(`Fetching games: page=${page}, query='${query}', genres=${genreFilters.join(',')}`);
    setIsLoading(true);
    
    // Add minimum loading time to show skeleton
    const minLoadTime = new Promise(resolve => setTimeout(resolve, 500));
    
    try {
      let result;
      
      if (query) {
        // Use hybrid search for queries
        setShowApiResults(true); // Always show API results view for searches
        result = await invoke("hybrid_search", { query, page, pageSize: gamesPerPage });
      } else {
        // Use regular catalogue browsing without search query
        setShowApiResults(false);
        if (genreFilters.length > 0) {
          // Use genre filtering
          result = await invoke("filter_games_by_genre", { genres: genreFilters, page, pageSize: gamesPerPage });
        } else {
          // Default: get all games
          result = await invoke("get_games", { page, pageSize: gamesPerPage });
        }
      }
      
      await minLoadTime;
      
      setGames(result.games);
      setTotalPages(Math.ceil(result.total / gamesPerPage));
    } catch (error) {
      console.error("Error fetching game data from backend:", error);
      setGames([]);
      setTotalPages(0);
    } finally {
      console.log("Setting loading to false");
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    // Fetch initial data
    fetchGames(currentPage, globalSearchQuery, selectedGenres);
  }, [currentPage, selectedGenres]); // Include selectedGenres dependency

  useEffect(() => {
    // Fetch genres once
    const fetchFilters = async () => {
      try {
        const genresRes = await invoke("get_all_genres");
        setGenres(genresRes);
      } catch (error) {
        console.error("Error fetching filters:", error);
      }
    };
    fetchFilters();
  }, []);

  // Function to extract App ID from Steam URL
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
  
    // Debounce search query changes
    useEffect(() => {
        if (globalSearchQuery.trim() === '') {
            // If search is cleared, fetch immediately
            setCurrentPage(1);
            fetchGames(1, '', selectedGenres);
            return;
        }

        // Check if the search query is a Steam URL or App ID
        const extractedAppId = extractAppIdFromUrl(globalSearchQuery);
        
        if (extractedAppId) {
            // Navigate directly to game detail page if it's a Steam URL or App ID
            navigate(`/game/${extractedAppId}`);
            return;
        }

        const handler = setTimeout(() => {
            setCurrentPage(1); // Reset to first page on new search
            fetchGames(1, globalSearchQuery, selectedGenres);
        }, 500); // 500ms debounce

        return () => {
            clearTimeout(handler);
        };
    }, [globalSearchQuery, navigate]); // Added navigate to deps

    // In useEffect for page/genre changes
    useEffect(() => {
        // This effect handles browsing and pagination, but not initial search query
        fetchGames(currentPage, '', selectedGenres);
    }, [currentPage, selectedGenres]);


  const handleGenreChange = (genre) => {
    setCurrentPage(1); // Reset to first page when genre filter changes
    setSelectedGenres(prev =>
      prev.includes(genre) ? prev.filter(g => g !== genre) : [...prev, genre]
    );
  };


  const filteredGenres = genres.filter(genre =>
    genre.toLowerCase().includes(genreFilter.toLowerCase())
  );

  // Filtering is now done on the backend, so we just use the games directly
  const gamesToShow = games;

  const hasFilters = genres.length > 0;

  const handlePageChange = (pageNumber) => {
    if (pageNumber >= 1 && pageNumber <= totalPages) {
      setCurrentPage(pageNumber);
    }
  };

  const getPageNumbers = () => {
    const pageNumbers = [];
    const maxVisiblePages = 5;
    if (totalPages <= maxVisiblePages) {
      for (let i = 1; i <= totalPages; i++) pageNumbers.push(i);
    } else {
      if (currentPage <= 3) {
        pageNumbers.push(1, 2, 3, 4, '...', totalPages);
      } else if (currentPage >= totalPages - 2) {
        pageNumbers.push(1, '...', totalPages - 3, totalPages - 2, totalPages - 1, totalPages);
      } else {
        pageNumbers.push(1, '...', currentPage - 1, currentPage, currentPage + 1, '...', totalPages);
      }
    }
    return pageNumbers;
  };

  const handleGameCardClick = (appId) => {
    navigate(`/game/${appId}`);
  };

  return (
    <div className="ui-page">
      <div className="ui-content">
        <div className="catalogue-container">
          <div className="catalogue-header">
            {/* Global search bar is in Header.jsx */}
          </div>
          <div className="catalogue-body">
            <div className={`game-list ${!hasFilters ? 'full-width' : ''}`}>
              {isLoading ? (
                /* Debug: Loading is true, showing skeleton */
                <div className={`catalogue-skeleton-container ${!hasFilters ? 'full-width' : ''}`}>
                  {[...Array(12)].map((_, index) => (
                    <div key={index} className="catalogue-game-card-skeleton">
                      <div className="game-image-skeleton"></div>
                      <div className="game-info-skeleton">
                        <div className="game-title-skeleton"></div>
                        <div className="game-meta-skeleton"></div>
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <>
                  {gamesToShow.length > 0 ? gamesToShow.map((game, index) => (
                    <div 
                      key={`${game.app_id}-${index}-${game.source || 'cat'}`} 
                      className="game-card"
                      onClick={() => handleGameCardClick(game.app_id)}
                    >
                      <img 
                        src={game.header_image?.includes('/header.jpg') 
                          ? game.header_image.replace('/header.jpg', '/library_hero.jpg')
                          : game.header_image
                        } 
                        alt={game.name} 
                        className="game-image" 
                        loading="lazy" 
                        width="231" 
                        height="87"
                        onError={(e) => {
                          // Fallback to header.jpg if library_hero.jpg fails
                          if (e.target.src.includes('/library_hero.jpg')) {
                            e.target.src = e.target.src.replace('/library_hero.jpg', '/header.jpg');
                          }
                        }}
                      />
                      <div className="game-info">
                        <h3>{game.name}</h3>
                      </div>
                    </div>
                  )) : (
                    <div className="no-results">
                      <p>No results found for "{globalSearchQuery}"</p>
                    </div>
                  )}
                  {totalPages > 1 && (
                    <div className="pagination">
                      <button className="pagination-btn" onClick={() => handlePageChange(currentPage - 1)} disabled={currentPage === 1}>Previous</button>
                      <div className="pagination-numbers">
                        {getPageNumbers().map((number, index) =>
                          number === '...' ? (
                            <span key={index} className="pagination-ellipsis">...</span>
                          ) : (
                            <button key={index} className={`pagination-number ${currentPage === number ? 'active' : ''}`} onClick={() => handlePageChange(number)}>{number}</button>
                          )
                        )}
                      </div>
                      <button className="pagination-btn" onClick={() => handlePageChange(currentPage + 1)} disabled={currentPage === totalPages}>Next</button>
                    </div>
                  )}
                </>
              )}
            </div>
            {hasFilters && (
              <div className="filter-sidebar">
                {/* Active Filters Display */}
                {selectedGenres.length > 0 && (
                  <div className="active-filters">
                    <div className="active-filters__header">
                      <h4>Active Filters</h4>
                      <button className="clear-all-btn" onClick={() => setSelectedGenres([])}>
                        Clear All
                      </button>
                    </div>
                    <div className="filter-pills">
                      {selectedGenres.map((genre) => (
                        <div key={genre} className="filter-pill">
                          <div className="filter-pill__orb"></div>
                          <span>{genre}</span>
                          <button 
                            className="filter-pill__remove"
                            onClick={() => setSelectedGenres(prev => prev.filter(g => g !== genre))}
                          >
                            ×
                          </button>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                <div className="filter-section">
                  <div className="filter-section__header">
                    <div className="filter-section__orb"></div>
                    <h4>Genres</h4>
                  </div>
                  
                  {selectedGenres.length > 0 ? (
                    <button className="filter-section__clear" onClick={() => setSelectedGenres([])}>
                      Clear {selectedGenres.length} filter{selectedGenres.length > 1 ? 's' : ''}
                    </button>
                  ) : (
                    <span className="filter-section__count">{genres.length} available</span>
                  )}
                  
                  <input 
                    type="text" 
                    className="filter-search" 
                    placeholder="Search genres..." 
                    value={genreFilter} 
                    onChange={(e) => setGenreFilter(e.target.value)} 
                  />
                  
                  <div className="filter-list">
                    {filteredGenres.slice(0, 10).map((genre) => (
                      <label key={genre} className="filter-item">
                        <input 
                          type="checkbox" 
                          checked={selectedGenres.includes(genre)} 
                          onChange={() => handleGenreChange(genre)} 
                        />
                        <span className="filter-label">{genre}</span>
                      </label>
                    ))}
                    {filteredGenres.length > 10 && (
                      <div className="filter-more">
                        +{filteredGenres.length - 10} more genres...
                      </div>
                    )}
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default Catalogue;

