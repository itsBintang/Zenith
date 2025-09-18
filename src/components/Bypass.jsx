import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FiShield, FiPlay, FiCheck, FiX, FiSearch, FiRefreshCw } from "react-icons/fi";
import { isTauri } from '@tauri-apps/api/core';
import "../styles/Bypass.css";

// Toast Notification Component
function ToastNotification({ message, type, onClose }) {
  useEffect(() => {
    const timer = setTimeout(() => {
      onClose();
    }, 5000); // Auto close after 5 seconds

    return () => clearTimeout(timer);
  }, [onClose]);

  return (
    <div className={`toast-notification ${type}`}>
      <div className="toast-content">
        <div className="toast-icon">
          {type === 'success' ? <FiCheck size={20} /> : <FiX size={20} />}
        </div>
        <div className="toast-message">{message}</div>
        <button className="toast-close" onClick={onClose}>
          <FiX size={16} />
        </button>
      </div>
    </div>
  );
}

// Game Card Component for Bypass
function GameCard({ game, onBypassClick, bypassStatus, isLoading }) {
  const isInstalled = bypassStatus[game.appId]?.installed;
  const isInstalling = bypassStatus[game.appId]?.installing;
  
  return (
    <div 
      className={`bypass-game-card ${isInstalled ? 'bypass-game-card--installed' : ''} ${isInstalling ? 'bypass-game-card--installing' : ''}`}
      onClick={() => !isLoading && !isInstalling && onBypassClick(game)}
      style={{ cursor: isLoading || isInstalling ? 'not-allowed' : 'pointer' }}
    >
      <div
        className="bypass-game-card__image"
        style={{ backgroundImage: `url(${game.image})` }}
      />
      <div className="bypass-game-card__overlay">
        <div className="bypass-game-card__content">
          <h3 className="bypass-game-card__title">{game.name}</h3>
        </div>
      </div>
    </div>
  );
}

function Bypass() {
  const [bypassGames, setBypassGames] = useState([]);
  const [isLoading, setIsLoading] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [filteredGames, setFilteredGames] = useState([]);
  const [notification, setNotification] = useState(null);
  
  // Bypass related states
  const [bypassStatus, setBypassStatus] = useState({});
  const [showBypassProgress, setShowBypassProgress] = useState(false);
  const [bypassProgress, setBypassProgress] = useState({ step: '', progress: 0, app_id: null });
  const [showBypassConfirmation, setShowBypassConfirmation] = useState(false);
  const [selectedGame, setSelectedGame] = useState(null);
  const [showLaunchPopup, setShowLaunchPopup] = useState(false);
  const [gameExecutablePath, setGameExecutablePath] = useState(null);
  const [gameExecutables, setGameExecutables] = useState([]);
  const [loadingExecutables, setLoadingExecutables] = useState(false);
  const [bypassNotes, setBypassNotes] = useState(null);
  const [isRefreshing, setIsRefreshing] = useState(false);

  // Load bypass games from JSON on component mount
  useEffect(() => {
    loadBypassGames();
  }, []);

  // Filter games based on search query
  useEffect(() => {
    if (searchQuery.trim() === "") {
      setFilteredGames(bypassGames);
    } else {
      const filtered = bypassGames.filter(game =>
        game.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        game.appId.toString().includes(searchQuery)
      );
      setFilteredGames(filtered);
    }
  }, [searchQuery, bypassGames]);

  // Bypass progress listener
  useEffect(() => {
    let unlistenFn = null;
    
    const setupListener = async () => {
      try {
        const isRunningInTauri = await isTauri();
        if (isRunningInTauri) {
          unlistenFn = await listen('bypass_progress', (event) => {
            const progress = event.payload;
            setBypassProgress(progress);
          });
        }
      } catch (error) {
        console.error('Failed to setup bypass progress listener:', error);
      }
    };

    setupListener();

    return () => {
      if (unlistenFn && typeof unlistenFn === 'function') {
        unlistenFn();
      }
    };
  }, []);

  const loadBypassGames = async () => {
    try {
      setIsLoading(true);
      
      // Load games from SQLite cache with 1 month TTL
      const gamesData = await invoke("get_bypass_games_cached");
      console.log('Loaded bypass games from cache:', gamesData);
      
      setBypassGames(gamesData);
      
      // Check bypass status for all games
      const statusPromises = gamesData.map(async (game) => {
        try {
          const isInstalled = await invoke("check_bypass_installed_command", { appId: game.appId });
          return { appId: game.appId, installed: isInstalled };
        } catch (error) {
          console.error(`Error checking bypass for ${game.name}:`, error);
          return { appId: game.appId, installed: false };
        }
      });

      const statusResults = await Promise.all(statusPromises);
      const statusMap = {};
      statusResults.forEach(result => {
        statusMap[result.appId] = {
          installed: result.installed,
          installing: false
        };
      });
      
      setBypassStatus(statusMap);
    } catch (error) {
      console.error('Error loading bypass games:', error);
      setNotification({
        message: 'Failed to load bypass games from cache',
        type: 'error'
      });
    } finally {
      setIsLoading(false);
    }
  };

  const refreshBypassGames = async () => {
    try {
      setIsRefreshing(true);
      
      // Force refresh from GitHub by clearing cache and fetching fresh data
      await invoke("refresh_bypass_games_cache");
      console.log('Bypass games cache refreshed from GitHub');
      
      // Reload bypass games
      const gamesData = await invoke("get_bypass_games_cached");
      console.log('Refreshed bypass games from GitHub:', gamesData);
      
      setBypassGames(gamesData);
      
      // Check bypass status for all games
      const statusPromises = gamesData.map(async (game) => {
        try {
          const isInstalled = await invoke("check_bypass_installed_command", { appId: game.appId });
          return { appId: game.appId, installed: isInstalled };
        } catch (error) {
          console.error(`Error checking bypass for ${game.name}:`, error);
          return { appId: game.appId, installed: false };
        }
      });

      const statusResults = await Promise.all(statusPromises);
      const statusMap = {};
      statusResults.forEach(result => {
        statusMap[result.appId] = {
          installed: result.installed,
          installing: false
        };
      });
      
      setBypassStatus(statusMap);
      
      setNotification({
        message: `Success refreshed ${gamesData.length} bypass games`,
        type: 'success'
      });
    } catch (error) {
      console.error('Error refreshing bypass games:', error);
      setNotification({
        message: `Failed to refresh bypass games: ${error}`,
        type: 'error'
      });
    } finally {
      setIsRefreshing(false);
    }
  };

  const handleBypassClick = async (game) => {
    if (bypassStatus[game.appId]?.installing) return;
  
    const isRunningInTauri = await isTauri();
    if (!isRunningInTauri) {
      setNotification({
        message: "Bypass is only available in desktop app",
        type: 'error'
      });
      return;
    }
  
    // Since we have static data, directly show confirmation popup
    console.log("🎯 Game selected for bypass:", game.name);
    console.log("🔧 Available bypasses:", game.bypasses);
    
    setSelectedGame(game);
    setShowBypassConfirmation(true);
  };

  const handleBypassConfirm = async () => {
    if (!selectedGame) return;
    
    setShowBypassConfirmation(false);
    
    setBypassStatus(prev => ({ 
      ...prev, 
      [selectedGame.appId]: { ...prev[selectedGame.appId], installing: true }
    }));
    setShowBypassProgress(true);
    setBypassProgress({ step: "Installing bypass...", progress: 0, app_id: selectedGame.appId });

    try {
      // Use the new direct bypass installation with static URLs
      const result = await invoke("install_bypass_direct", { 
        appId: selectedGame.appId,
        gameName: selectedGame.name,
        bypasses: selectedGame.bypasses
      });

      console.log("🔍 Bypass installation result:", result);

      if (result && result.success) {
        setNotification({
          message: "Bypass installed successfully!",
          type: 'success'
        });
        
        setBypassStatus(prev => ({
          ...prev,
          [selectedGame.appId]: {
            installing: false,
            installed: true
          }
        }));

        // Show launch popup if game executable found
        console.log("🔍 Checking launch conditions:");
        console.log("  - result.should_launch:", result.should_launch);
        console.log("  - result.game_executable_path:", result.game_executable_path);
        
        if (result.game_executable_path) {
          console.log("🎮 Game executable path found:", result.game_executable_path);
          setGameExecutablePath(result.game_executable_path);
          await loadGameExecutables(result.game_executable_path);
          console.log("🚀 Setting showLaunchPopup to true");
          setShowLaunchPopup(true);
          // Don't reset selectedGame here, keep it for launch popup
        } else {
          console.log("❌ No executable path provided");
          setSelectedGame(null);
        }
      } else {
        setNotification({
          message: `Bypass installation failed: ${result?.message || 'Unknown error'}`,
          type: 'error'
        });
        setBypassStatus(prev => ({ 
          ...prev, 
          [selectedGame.appId]: { ...prev[selectedGame.appId], installing: false }
        }));
        setSelectedGame(null);
      }
    } catch (error) {
      console.error("Bypass installation error:", error);
      setNotification({
        message: `Failed to install bypass: ${error}`,
        type: 'error'
      });
      setBypassStatus(prev => ({ 
        ...prev, 
        [selectedGame.appId]: { ...prev[selectedGame.appId], installing: false }
      }));
      setSelectedGame(null);
    } finally {
      setShowBypassProgress(false);
    }
  };

  const loadGameExecutables = async (gamePath) => {
    setLoadingExecutables(true);
    try {
      console.log("🔍 Loading bypass notes and executables from:", gamePath);
      const bypassNotes = await invoke("get_bypass_notes", { 
        gamePath: gamePath
      });
      
      console.log("📝 Bypass notes:", bypassNotes);
      console.log("📄 Executable list:", bypassNotes.exe_list);
      
      // Set executables from bypass notes
      setGameExecutables(bypassNotes.exe_list || []);
      setBypassNotes(bypassNotes);
      
      // If there are notes, show them in console
      if (bypassNotes.has_notes) {
        console.log("📋 Instructions:", bypassNotes.instructions);
        if (bypassNotes.recommended_exe) {
          console.log("🎯 Recommended exe:", bypassNotes.recommended_exe);
        }
      }
    } catch (error) {
      console.error("Error loading bypass notes:", error);
      setGameExecutables([]);
    } finally {
      setLoadingExecutables(false);
    }
  };

  const handleLaunchSelectedExecutable = async (executablePath, executableName) => {
    try {
      console.log("🎮 User selected executable:", executableName);
      console.log("📁 Path:", executablePath);
      
      // Use selectedGame if available, otherwise use a generic game name
      const gameName = selectedGame?.name || "Game";
      
      const result = await invoke("confirm_and_launch_game", { 
        executablePath: executablePath,
        gameName: gameName
      });
      
      setNotification({
        message: `${executableName} launched successfully! Game is starting with bypass enabled.`,
        type: 'success'
      });
      setShowLaunchPopup(false);
      setSelectedGame(null);
      
      console.log("✅ Launch result:", result);
    } catch (error) {
      console.error("Launch game error:", error);
      setNotification({
        message: `Failed to launch ${executableName}: ${error}`,
        type: 'error'
      });
    }
  };


  const closeNotification = () => {
    setNotification(null);
  };

  // Debug logging for launch popup state
  React.useEffect(() => {
    console.log("🔍 Launch popup state changed:");
    console.log("  - showLaunchPopup:", showLaunchPopup);
    console.log("  - selectedGame:", selectedGame);
    console.log("  - gameExecutables:", gameExecutables);
  }, [showLaunchPopup, selectedGame, gameExecutables]);

  return (
    <div className="bypass-page">
      {notification && (
        <ToastNotification
          message={notification.message}
          type={notification.type}
          onClose={closeNotification}
        />
      )}

      {/* Header */}
      <div className="bypass-header">
        <div className="bypass-header__left">
          <h1>Bypass</h1>
        </div>
        <div className="bypass-header__right">
          <div className="bypass-search">
            <FiSearch className="bypass-search__icon" />
            <input
              type="text"
              placeholder="Search games..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="bypass-search__input"
            />
          </div>
          <button 
            className={`bypass-refresh-btn ${isRefreshing ? 'loading' : ''}`}
            onClick={refreshBypassGames}
            disabled={isRefreshing || isLoading}
            title="Refresh bypass games from GitHub"
          >
            <FiRefreshCw size={16} />
          </button>
        </div>
      </div>

      {/* Games Grid */}
      <div className="bypass-content">
        {isLoading ? (
          <div className="bypass-loading">
            <div className="spinner"></div>
            <p>Loading bypass games...</p>
          </div>
        ) : filteredGames.length === 0 ? (
          <div className="bypass-empty">
            <FiShield size={64} className="bypass-empty__icon" />
            <h3>No games found</h3>
            <p>
              {searchQuery 
                ? "No games match your search criteria" 
                : "No bypass games available. Check back later!"
              }
            </p>
          </div>
        ) : (
          <div className="bypass-games-grid">
            {filteredGames.map((game) => (
              <GameCard
                key={game.appId}
                game={game}
                onBypassClick={handleBypassClick}
                bypassStatus={bypassStatus}
                isLoading={isLoading}
              />
            ))}
          </div>
        )}
      </div>

      {/* Bypass Confirmation Modal */}
      {showBypassConfirmation && selectedGame && (
        <div className="modal-overlay">
          <div className="bypass-confirmation-modal">
            <div className="confirmation-content">
              <FiShield className="bypass-icon" />
              <h3>🎯 Bypass Available!</h3>
              <p>
                Bypass tersedia untuk <strong>{selectedGame.name}</strong>.
                Silakan konfirmasi untuk melanjutkan instalasi.
              </p>
              <div className="bypass-confirmation__warning">
                <p><strong>Penting:</strong> Bypass akan menimpa file game. Pastikan game sudah diinstal dengan benar.</p>
              </div>
            </div>
            <div className="bypass-confirmation__actions">
              <button 
                className="bypass-confirmation__confirm-button" 
                onClick={handleBypassConfirm}
              >
                Yes, Install
              </button>
              <button 
                className="bypass-confirmation__cancel-button" 
                onClick={() => {
                  setShowBypassConfirmation(false);
                  setSelectedGame(null);
                }}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Bypass Progress Modal */}
      {showBypassProgress && (
        <div className="modal-overlay">
          <div className="bypass-progress-modal">
            <h3>Installing Bypass</h3>
            <div className="progress-container">
              <div className="progress-bar">
                <div 
                  className="progress-fill" 
                  style={{ width: `${bypassProgress.progress}%` }}
                />
              </div>
              <div className="progress-text">
                <p className="progress-step">{bypassProgress.step}</p>
                <span className="progress-percentage">{bypassProgress.progress.toFixed(1)}%</span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Launch Game Popup */}
      {showLaunchPopup && (
        <div className="modal-overlay">
          <div className="launch-popup-modal">
            <div className="launch-popup-content">
              <h3>Bypass Terinstal</h3>
              
              <div className="launch-popup-info simplified">
                <p>
                  <strong>Penting:</strong> Untuk mengaktifkan bypass, game harus dijalankan <strong>pertama kali</strong> dari sini. Jangan buka melalui Steam atau shortcut lain.
                </p>
              </div>

              <div className="launch-popup-question">
                <h4>Pilih executable untuk launch game:</h4>
              </div>

              <div className="executable-list">
                {loadingExecutables ? (
                  <div className="loading-executables">
                    <div className="spinner"></div>
                    <span>Scanning executable files...</span>
                  </div>
                ) : gameExecutables.length > 0 ? (
                  gameExecutables.map((exe, index) => (
                      <button
                        key={exe.path}
                        className="executable-item"
                        onClick={() => handleLaunchSelectedExecutable(exe.path, exe.name)}
                      >
                        <div className="executable-info">
                          <div className="executable-name">
                            <span className="exe-name">{exe.name}</span>
                          </div>
                          <div className="executable-size">{exe.size_mb.toFixed(1)} MB</div>
                        </div>
                        <FiPlay className="launch-icon-small" />
                      </button>
                    ))
                ) : (
                  <div className="no-executables">
                    <p>No executable files found in game directory.</p>
                  </div>
                )}
              </div>
              
              <div className="launch-popup-actions">
                <button 
                  className="launch-button secondary"
                  onClick={() => {
                    setShowLaunchPopup(false);
                    setSelectedGame(null);
                  }}
                >
                  Cancel
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default Bypass;
