import React, { useState, useEffect, useCallback, useRef } from 'react';
import { useSearchParams, useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import './Catalogue.css';

const Catalogue = () => {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const catalogueRef = useRef(null);
  const [games, setGames] = useState([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState(null);
  const [totalCount, setTotalCount] = useState(0);
  const [pagination, setPagination] = useState(null);
  
  // Get search query and page from URL parameters
  const searchQuery = searchParams.get('search') || '';
  const currentPage = parseInt(searchParams.get('page')) || 1;

  // Fetch catalogue games using Hydra API (with optional search)
  const fetchCatalogueGames = useCallback(async (page = 1, query = '') => {
    setIsLoading(true);
    setError(null);
    
    try {
      let response;
      
      if (query && query.trim().length >= 2) {
        console.log(`🔍 Searching catalogue for: "${query}" (page: ${page})`);
        response = await invoke('search_catalogue_games', {
          query: query.trim(),
          page: page,
          itemsPerPage: 20
        });
      } else {
        console.log(`📚 Fetching catalogue page ${page}...`);
        response = await invoke('get_paginated_catalogue', { 
          page: page,
          itemsPerPage: 20
        });
      }
      
      
      
      setGames(response.games || []);
      setPagination(response.pagination);
      setTotalCount(response.pagination?.total_items || 0);
      
    } catch (error) {
      console.error('❌ Failed to fetch catalogue games:', error);
      setError(`Failed to load catalogue: ${error}`);
      setGames([]);
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Test Hydra connection
  const testHydraConnection = useCallback(async () => {
    try {
      await invoke('test_hydra_connection');
    } catch (error) {
      console.warn('API connection failed:', error);
    }
  }, []);

  // Load data when URL parameters change
  useEffect(() => {
    testHydraConnection();
    fetchCatalogueGames(currentPage, searchQuery);
  }, [currentPage, searchQuery, fetchCatalogueGames, testHydraConnection]);

  // Smooth scroll to top when data changes (after loading is complete)
  useEffect(() => {
    if (!isLoading && catalogueRef.current) {
      // Small delay to ensure content is rendered
      setTimeout(() => {
        if (catalogueRef.current) {
          catalogueRef.current.scrollTo({
            top: 0,
            behavior: 'smooth'
          });
        }
      }, 100);
    }
  }, [isLoading, currentPage, searchQuery]);

  // Handle page change
  const handlePageChange = (newPage) => {
    if (newPage !== currentPage && newPage >= 1 && pagination?.total_pages && newPage <= pagination.total_pages) {
      // Update URL parameters using navigate (scroll will happen after data loads)
      const newParams = new URLSearchParams();
      if (searchQuery) {
        newParams.set('search', searchQuery);
      }
      newParams.set('page', newPage.toString());
      
      navigate(`/catalogue?${newParams.toString()}`);
    }
  };

  // Handle game click
  const handleGameClick = (game) => {
    const appId = game.object_id;
    
    if (appId) {
      navigate(`/game/${appId}`);
    }
  };

  if (isLoading) {
    return (
      <div className="ui-page">
        <div className="catalogue-body" ref={catalogueRef}>
          {/* Header with active filters */}
          <div className="catalogue-header">
            <div className="catalogue-filters-wrapper">
              {searchQuery && (
                <div className="filter-item">
                  <div className="filter-item__orb" style={{ backgroundColor: "#3e62c0" }}></div>
                  <span>Search: "{searchQuery}"</span>
                  <button 
                    className="filter-item__remove-button"
                    onClick={() => navigate('/catalogue')}
                    title="Clear search"
                  >
                    ×
                  </button>
                </div>
              )}
            </div>
          </div>

          {/* Main content area with skeleton */}
          <div className="catalogue-content">
            {/* Games container with skeleton */}
            <div className="catalogue-games-container">
              {Array.from({ length: 10 }, (_, index) => (
                <div key={index} className="game-item-skeleton">
                  <div className="game-item-skeleton__cover"></div>
                  <div className="game-item-skeleton__details">
                    <div className="game-item-skeleton__title"></div>
                    <div className="game-item-skeleton__genres"></div>
                  </div>
                </div>
              ))}
            </div>

            {/* Filters sidebar skeleton */}
            <div className="catalogue-filters-container">
              <div className="catalogue-filters-sections">
                {/* Genres Filter Skeleton */}
                <div className="filter-section">
                  <div className="filter-section__header">
                    <div className="filter-section__orb" style={{ backgroundColor: "hsl(262deg 50% 47%)" }}></div>
                    <h3 className="filter-section__title">Genres</h3>
                  </div>
                  <div className="filter-section-skeleton__count"></div>
                  <div className="filter-section-skeleton__search"></div>
                  <div className="filter-section-skeleton__items">
                    {Array.from({ length: 5 }, (_, index) => (
                      <div key={index} className="filter-section-skeleton__item"></div>
                    ))}
                  </div>
                </div>

                {/* Tags Filter Skeleton */}
                <div className="filter-section">
                  <div className="filter-section__header">
                    <div className="filter-section__orb" style={{ backgroundColor: "hsl(95deg 50% 20%)" }}></div>
                    <h3 className="filter-section__title">Tags</h3>
                  </div>
                  <div className="filter-section-skeleton__count"></div>
                  <div className="filter-section-skeleton__search"></div>
                  <div className="filter-section-skeleton__items">
                    {Array.from({ length: 5 }, (_, index) => (
                      <div key={index} className="filter-section-skeleton__item"></div>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="ui-page">
        <div className="catalogue-body" ref={catalogueRef}>
          <div className="catalogue-error">
            <h3>Error Loading Catalogue</h3>
            <p>{error}</p>
            <button onClick={() => fetchCatalogueGames(currentPage)} className="retry-button">
              Try Again
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="ui-page">
      <div className="catalogue-body" ref={catalogueRef}>
        {/* Header with active filters */}
        <div className="catalogue-header">
          <div className="catalogue-filters-wrapper">
            {searchQuery && (
              <div className="filter-item">
                <div className="filter-item__orb" style={{ backgroundColor: "#3e62c0" }}></div>
                <span>Search: "{searchQuery}"</span>
                <button 
                  className="filter-item__remove-button"
                  onClick={() => navigate('/catalogue')}
                  title="Clear search"
                >
                  ×
                </button>
              </div>
            )}
          </div>
        </div>

        {/* Main content area */}
        <div className="catalogue-content">
          {/* Games container */}
          <div className="catalogue-games-container">
            {games.length > 0 ? (
              <>
                {games.map((game) => (
                  <div
                    key={game.id || game.object_id}
                    className="game-item"
                    onClick={() => handleGameClick(game)}
                  >
                    {/* Game cover image */}
                    {game.library_image_url ? (
                      <img
                        src={game.library_image_url}
                        alt={game.title}
                        className="game-item__cover"
                        loading="lazy"
                        onError={(e) => {
                          e.target.style.display = 'none';
                          e.target.nextElementSibling.style.display = 'flex';
                        }}
                      />
                    ) : null}
                    <div 
                      className="game-item__cover-placeholder" 
                      style={{ display: game.library_image_url ? 'none' : 'flex' }}
                    >
                      <span>?</span>
                    </div>

                    {/* Game details */}
                    <div className="game-item__details">
                      <span className="game-item__title">{game.title}</span>
                      <span className="game-item__genres">
                        {game.genres && game.genres.length > 0 ? game.genres.join(', ') : 'No genres'}
                      </span>
                    </div>

                    {/* Add to library button placeholder */}
                    <div className="game-item__plus-wrapper" title="Add to library">
                      <span>+</span>
                    </div>
                  </div>
                ))}

                {/* Pagination */}
                <div className="catalogue-pagination-container">
                  <span className="catalogue-result-count">
                    {totalCount.toLocaleString()} {searchQuery ? 'games found' : 'games available'}
                  </span>

                  {pagination && pagination.total_pages > 1 && (
                    <div className="catalogue-pagination">
                      <button
                        className="pagination-btn"
                        onClick={() => handlePageChange(currentPage - 1)}
                        disabled={!pagination.has_prev_page}
                      >
                        Previous
                      </button>
                      
                      <span className="pagination-info">
                        Page {currentPage} of {pagination.total_pages}
                      </span>
                      
                      <button
                        className="pagination-btn"
                        onClick={() => handlePageChange(currentPage + 1)}
                        disabled={!pagination.has_next_page}
                      >
                        Next
                      </button>
                    </div>
                  )}
                </div>
              </>
            ) : (
              <div className="no-games">
                <h3>No games found</h3>
                <p>{searchQuery ? 'Try adjusting your search' : 'No games available'}</p>
              </div>
            )}
          </div>

          {/* Filters sidebar */}
          <div className="catalogue-filters-container">
            <div className="catalogue-filters-sections">
              {/* Genres Filter */}
              <div className="filter-section">
                <div className="filter-section__header">
                  <div className="filter-section__orb" style={{ backgroundColor: "hsl(262deg 50% 47%)" }}></div>
                  <h3 className="filter-section__title">Genres</h3>
                </div>
                <span className="filter-section__count">29 available</span>
                <input 
                  type="text" 
                  className="filter-section__search" 
                  placeholder="Filter..."
                />
                <div className="filter-section__items">
                  <div className="filter-section__item">
                    <input type="checkbox" id="action" />
                    <label htmlFor="action">Action</label>
                  </div>
                  <div className="filter-section__item">
                    <input type="checkbox" id="adventure" />
                    <label htmlFor="adventure">Adventure</label>
                  </div>
                  <div className="filter-section__item">
                    <input type="checkbox" id="rpg" />
                    <label htmlFor="rpg">RPG</label>
                  </div>
                </div>
              </div>

              {/* Tags Filter */}
              <div className="filter-section">
                <div className="filter-section__header">
                  <div className="filter-section__orb" style={{ backgroundColor: "hsl(95deg 50% 20%)" }}></div>
                  <h3 className="filter-section__title">Tags</h3>
                </div>
                <span className="filter-section__count">447 available</span>
                <input 
                  type="text" 
                  className="filter-section__search" 
                  placeholder="Filter..."
                />
                <div className="filter-section__items">
                  <div className="filter-section__item">
                    <input type="checkbox" id="singleplayer" />
                    <label htmlFor="singleplayer">Singleplayer</label>
                  </div>
                  <div className="filter-section__item">
                    <input type="checkbox" id="multiplayer" />
                    <label htmlFor="multiplayer">Multiplayer</label>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default Catalogue;