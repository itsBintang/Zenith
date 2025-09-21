import React, { useState, useEffect, useMemo } from 'react';
import { useNavigate, useOutletContext } from 'react-router-dom';
import Papa from 'papaparse';
import { invoke } from '@tauri-apps/api/core';
import './Catalogue.css';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faSearch } from '@fortawesome/free-solid-svg-icons';
import { FiSearch } from 'react-icons/fi';

const Catalogue = () => {
  const navigate = useNavigate();
  const { globalSearchQuery, setGlobalSearchQuery } = useOutletContext();
  const [games, setGames] = useState([]);
  const [apiResults, setApiResults] = useState([]);
  const [showApiResults, setShowApiResults] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isSearching, setIsSearching] = useState(false);
  
  // Local search state is no longer needed
  // const [searchQuery, setSearchQuery] = useState(""); 

  const [genres, setGenres] = useState([]);
  const [tags, setTags] = useState([]);
  const [selectedGenres, setSelectedGenres] = useState([]);
  const [selectedTags, setSelectedTags] = useState([]);
  const [genreFilter, setGenreFilter] = useState('');
  const [tagFilter, setTagFilter] = useState('');
  const [currentPage, setCurrentPage] = useState(1);
  const gamesPerPage = 50;

  // Function to extract App ID from Steam URL
  const extractAppIdFromUrl = (input) => {
    if (!input) return null;
    
    // Check if input is already a pure App ID (numeric)
    if (/^\d+$/.test(input.trim())) {
      return input.trim();
    }
    
    // Steam URL patterns:
    // https://store.steampowered.com/app/851850/DRAGON_BALL_Z_KAKAROT/
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

  useEffect(() => {
    const fetchGames = async () => {
      setIsLoading(true);
      try {
        Papa.parse('https://raw.githubusercontent.com/itsBintang/SteamDB/main/steamdb.csv', {
          download: true,
          header: true,
          complete: (results) => {
            const gameData = results.data.map(game => ({
              appId: game['applogo href']?.split('/')[4],
              name: game.b,
              imageUrl: game['applogo src'],
              genres: game.genres ? game.genres.split(',').map(g => g.trim()).filter(g => g) : [],
              tags: game.tags ? game.tags.split(',').map(t => t.trim()).filter(t => t) : [],
            })).filter(game => game.name && game.imageUrl);

            setGames(gameData);

            const allGenres = [...new Set(gameData.flatMap(game => game.genres))].sort();
            const allTags = [...new Set(gameData.flatMap(game => game.tags))].sort();

            setGenres(allGenres);
            setTags(allTags);
            setIsLoading(false);
          },
          error: (error) => {
            console.error("Error parsing CSV:", error);
            setIsLoading(false);
          }
        });
      } catch (error) {
        console.error("Error fetching game data:", error);
        setIsLoading(false);
      }
    };

    fetchGames();
  }, []);

  useEffect(() => {
    const searchInCatalogueAndAPI = async () => {
      if (!globalSearchQuery.trim()) {
        setShowApiResults(false);
        setApiResults([]);
        setIsSearching(false);
        return;
      }

      setIsSearching(true);
      setShowApiResults(false);

      // Extract App ID from URL if applicable
      const steamUrlRegex = /store\.steampowered\.com\/app\/(\d+)/;
      const match = globalSearchQuery.match(steamUrlRegex);
      const extractedAppId = match ? match[1] : (Number.isInteger(Number(globalSearchQuery)) ? globalSearchQuery : null);
      
      if (extractedAppId) {
        navigate(`/game/${extractedAppId}`);
        return;
      }
      
      // Filter from local CSV data first
      const catalogueResults = games.filter(game =>
        game.name.toLowerCase().includes(globalSearchQuery.toLowerCase())
      );

      if (catalogueResults.length > 0) {
        // This part might need adjustment depending on how you want to show hybrid results.
        // For now, if CSV has results, we don't call the API.
        // Or we can decide to always call API and merge results.
        // Let's stick to the logic: if catalogue has results, show them. API is a fallback.
        setShowApiResults(false); // Ensure we're showing catalogue results
      } else {
        // If no results in CSV, fallback to API
        try {
          const res = await invoke("search_games_api", { query: globalSearchQuery });
          const cleanedResults = res.filter(game => 
            game.type === "game" || game.type === "application"
          );
          setApiResults(cleanedResults);
          setShowApiResults(true);
        } catch (error) {
          console.error("API search failed:", error);
          setApiResults([]);
          setShowApiResults(true); // Show that we tried and failed (empty results)
        }
      }
      
      setIsSearching(false);
    };

    const debounceSearch = setTimeout(() => {
      searchInCatalogueAndAPI();
    }, 500); // 500ms debounce delay

    return () => clearTimeout(debounceSearch);
  }, [globalSearchQuery, games, navigate]);

  const filteredGames = useMemo(() => {
    if (showApiResults) return [];

    let results = games;

    if (globalSearchQuery.trim()) {
      results = results.filter(game =>
        game.name.toLowerCase().includes(globalSearchQuery.toLowerCase())
      );
    }

    if (selectedGenres.length > 0) {
      results = results.filter(game =>
        selectedGenres.every(genre => game.genres.includes(genre))
      );
    }

    if (selectedTags.length > 0) {
      results = results.filter(game =>
        selectedTags.every(tag => game.tags.includes(tag))
      );
    }

    return results;
  }, [games, globalSearchQuery, selectedGenres, selectedTags, showApiResults]);

  const handleGenreChange = (genre) => {
    setSelectedGenres(prev =>
      prev.includes(genre) ? prev.filter(g => g !== genre) : [...prev, genre]
    );
  };

  const handleTagChange = (tag) => {
    setSelectedTags(prev =>
      prev.includes(tag) ? prev.filter(t => t !== tag) : [...prev, tag]
    );
  };

  const filteredGenres = useMemo(() =>
    genres.filter(genre =>
      genre.toLowerCase().includes(genreFilter.toLowerCase())
    ), [genres, genreFilter]
  );

  const filteredTags = useMemo(() =>
    tags.filter(tag =>
      tag.toLowerCase().includes(tagFilter.toLowerCase())
    ), [tags, tagFilter]
  );

  const hasFilters = genres.length > 0 || tags.length > 0;

  // Pagination logic based on filteredGames
  const totalPages = Math.ceil(filteredGames.length / gamesPerPage);
  const indexOfLastGame = currentPage * gamesPerPage;
  const indexOfFirstGame = indexOfLastGame - gamesPerPage;
  const currentGames = filteredGames.slice(indexOfFirstGame, indexOfLastGame);

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

  const handleApiResultClick = (result) => {
    navigate(`/game/${result.app_id}`);
  };

  const handleGameCardClick = (appId) => {
    navigate(`/game/${appId}`);
  };

  return (
    <div className="ui-page">
      <div className="ui-content">
        <div className="catalogue-container">
          <div className="catalogue-header">
            {/* The search bar is now removed from here and handled globally by Header.jsx */}
          </div>
          <div className="catalogue-body">
            <div className={`game-list ${!hasFilters ? 'full-width' : ''}`}>
              {isLoading ? (
                <>
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
          </>
              ) : showApiResults ? (
                <>
                  {apiResults.map((game, index) => (
                    <div 
                      key={`api-${game.app_id}-${index}`} 
                      className="game-card"
                      onClick={() => handleApiResultClick(game)}
                    >
                      <img src={game.header_image} alt={game.name} className="game-image" loading="lazy" width="231" height="87" />
                      <div className="game-info">
                        <h3>{game.name}</h3>
                        <p></p>
                      </div>
                    </div>
                  ))}
                </>
              ) : (
                <>
                  {currentGames.map((game, index) => (
                    <div 
                      key={`${game.appId}-${index}`} 
                      className="game-card"
                      onClick={() => handleGameCardClick(game.appId)}
                    >
                      <img src={game.imageUrl} alt={game.name} className="game-image" loading="lazy" width="231" height="87" />
                      <div className="game-info">
                        <h3>{game.name}</h3>
                        <p>{[...game.genres, ...game.tags].slice(0, 3).join(', ')}</p>
                      </div>
                    </div>
                  ))}
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
                <div className="filter-section">
                  <h4><span className="dot"></span>Genres</h4>
                  <p>{genres.length} available</p>
                  <input type="text" placeholder="Filter..." value={genreFilter} onChange={(e) => setGenreFilter(e.target.value)} />
                  <div className="checkbox-list">
                    {filteredGenres.map((genre, index) => (
                      <label key={index}>
                        <input type="checkbox" checked={selectedGenres.includes(genre)} onChange={() => handleGenreChange(genre)} />
                        {genre}
                      </label>
                    ))}
                  </div>
                </div>
                <div className="filter-section">
                  <h4><span className="dot"></span>Tags</h4>
                  <p>{tags.length} available</p>
                  <input type="text" placeholder="Filter..." value={tagFilter} onChange={(e) => setTagFilter(e.target.value)} />
                  <div className="checkbox-list">
                    {filteredTags.map((tag, index) => (
                      <label key={index}>
                        <input type="checkbox" checked={selectedTags.includes(tag)} onChange={() => handleTagChange(tag)} />
                        {tag}
                      </label>
                    ))}
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
