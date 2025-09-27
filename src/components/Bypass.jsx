import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FiShield, FiPlay, FiCheck, FiX, FiSearch, FiRefreshCw, FiFolder } from "react-icons/fi";
import { isTauri } from '@tauri-apps/api/core';
import { open } from "@tauri-apps/plugin-dialog";
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
  const [currentDownloadId, setCurrentDownloadId] = useState(null);
  const [downloadPaused, setDownloadPaused] = useState(false);
  const [showLaunchPopup, setShowLaunchPopup] = useState(false);
  const [gameExecutablePath, setGameExecutablePath] = useState(null);
  const [gameExecutables, setGameExecutables] = useState([]);
  const [loadingExecutables, setLoadingExecutables] = useState(false);
  const [bypassNotes, setBypassNotes] = useState(null);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [gameInstallationStatus, setGameInstallationStatus] = useState({
    isInstalled: false,
    isChecking: false,
    gamePath: "",
    isManualPath: false
  });

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
          
          // Capture download ID when available
          if (progress.download_id && !currentDownloadId) {
            setCurrentDownloadId(progress.download_id);
          }
          
          // Handle completion (100% progress)
          if (progress.progress >= 100) {
            console.log('🎉 Bypass installation completed!');
            setShowBypassProgress(false);
            setShowBypassConfirmation(false);
            setCurrentDownloadId(null);
            setDownloadPaused(false);
            
            // Update bypass status to installed
            if (progress.app_id) {
              setBypassStatus(prev => ({ 
                ...prev, 
                [progress.app_id]: { installed: true, installing: false }
              }));
            }
            
            // Show success notification
            setNotification({
              message: `Bypass activation completed successfully!`,
              type: 'success'
            });
          }
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

  // Check if game is installed when card is clicked
  const checkGameInstallation = async (appId) => {
    setGameInstallationStatus(prev => ({ ...prev, isChecking: true }));
    
    try {
      const gameInfo = await invoke("get_game_installation_info", { appId });
      console.log("🎯 Game installation info:", gameInfo);
      
      setGameInstallationStatus({
        isInstalled: true,
        isChecking: false,
        gamePath: gameInfo.install_path,
        isManualPath: false
      });
      
      return true;
    } catch (error) {
      console.log("❌ Game not installed:", error);
      
      setGameInstallationStatus({
        isInstalled: false,
        isChecking: false,
        gamePath: "",
        isManualPath: false
      });
      
      return false;
    }
  };

  // Browse manual game path
  const browseManualGamePath = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (selected && typeof selected === "string") {
        console.log("📁 Manual path selected:", selected);
        
        setGameInstallationStatus({
          isInstalled: true,
          isChecking: false,
          gamePath: selected,
          isManualPath: true
        });
        
        setNotification({
          message: "Manual game path set successfully!",
          type: 'success'
        });
      }
    } catch (error) {
      console.error("Folder selection failed:", error);
      setNotification({
        message: "Failed to open folder browser",
        type: 'error'
      });
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
  
    // Check game installation when card is clicked
    console.log("🎯 Game selected for bypass:", game.name);
    console.log("🔧 Available bypasses:", game.bypasses);
    
    setSelectedGame(game);
    setShowBypassConfirmation(true);
    
    // Check if game is installed
    await checkGameInstallation(game.appId);
  };

  const handleBypassConfirm = async () => {
    if (!selectedGame || (!gameInstallationStatus.isInstalled && !gameInstallationStatus.gamePath)) return;
    
    // Jangan tutup modal, cukup tampilkan progress section
    // setShowBypassConfirmation(false);
    
    setBypassStatus(prev => ({ 
      ...prev, 
      [selectedGame.appId]: { ...prev[selectedGame.appId], installing: true }
    }));
    setShowBypassProgress(true);
    setBypassProgress({ step: "Installing bypass...", progress: 0, app_id: selectedGame.appId });

    try {
      // Use the universal bypass installation method
      // Pass manual path if available
      const installParams = { 
        appId: selectedGame.appId
      };
      
      if (gameInstallationStatus.isManualPath && gameInstallationStatus.gamePath) {
        installParams.manualGamePath = gameInstallationStatus.gamePath;
      }
      
      const result = await invoke("install_bypass", installParams);

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

        // Don't show launch popup - bypass installation complete
        console.log("🔍 Bypass installation complete");
        console.log("  - result.should_launch:", result.should_launch);
        console.log("  - result.game_executable_path:", result.game_executable_path);
        
        // Just finish - keep modal open untuk user melihat hasil
        // User bisa close manual dengan klik X atau ESC
      } else {
        setNotification({
          message: `Bypass installation failed: ${result?.message || 'Unknown error'}`,
          type: 'success' // Changed to success since game check is now done earlier
        });
        setBypassStatus(prev => ({ 
          ...prev, 
          [selectedGame.appId]: { ...prev[selectedGame.appId], installing: false }
        }));
        setSelectedGame(null);
      }
    } catch (error) {
      console.error("Bypass installation error:", error);
      // Don't show error notification for game not found - it's handled in UI now
      setBypassStatus(prev => ({ 
        ...prev, 
        [selectedGame.appId]: { ...prev[selectedGame.appId], installing: false }
      }));
      setSelectedGame(null);
    } finally {
      setShowBypassProgress(false);
      setCurrentDownloadId(null);
      setDownloadPaused(false);
    }
  };

  const handlePauseDownload = async () => {
    if (!currentDownloadId) return;
    
    try {
      if (downloadPaused) {
        // Resume download
        await invoke('resume_download', { downloadId: currentDownloadId });
        setDownloadPaused(false);
        setNotification({
          message: 'Download resumed',
          type: 'success'
        });
      } else {
        // Pause download
        await invoke('pause_download', { downloadId: currentDownloadId });
        setDownloadPaused(true);
        setNotification({
          message: 'Download paused',
          type: 'success'
        });
      }
    } catch (error) {
      console.error('Failed to pause/resume download:', error);
      setNotification({
        message: `Failed to ${downloadPaused ? 'resume' : 'pause'} download: ${error}`,
        type: 'error'
      });
    }
  };

  const handleCancelDownload = async () => {
    if (!currentDownloadId) return;
    
    try {
      await invoke('cancel_download', { downloadId: currentDownloadId });
      
      // Reset states
      setShowBypassProgress(false);
      setCurrentDownloadId(null);
      setDownloadPaused(false);
      setBypassStatus(prev => ({ 
        ...prev, 
        [selectedGame.appId]: { ...prev[selectedGame.appId], installing: false }
      }));
      
      setNotification({
        message: 'Download cancelled',
        type: 'success'
      });
    } catch (error) {
      console.error('Failed to cancel download:', error);
      setNotification({
        message: `Failed to cancel download: ${error}`,
        type: 'error'
      });
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
    <div className="ui-page">
      <div className="ui-content">
        <div className="bypass-container">
          {notification && (
            <ToastNotification
              message={notification.message}
              type={notification.type}
              onClose={closeNotification}
            />
          )}

          {/* Header is now managed globally by Header.jsx */}
          
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
            <div className="denuvo-modal-overlay">
              <div className="denuvo-activation-modal">
                {/* Header */}
                <div className="denuvo-header">
                  <h2 className="denuvo-header-title">Bypass Activation</h2>
                  <p className="denuvo-header-subtitle">Manage Bypass activation for this game</p>
                  <button 
                    className="denuvo-close-btn"
                    onClick={() => {
                      setShowBypassConfirmation(false);
                      setSelectedGame(null);
                      setGameInstallationStatus({
                        isInstalled: false,
                        isChecking: false,
                        gamePath: "",
                        isManualPath: false
                      });
                    }}
                  >
                    <FiX size={20} />
                  </button>
                </div>

                {/* Content */}
                <div className="denuvo-content">
                  {/* Game Card */}
                  <div className="denuvo-game-card">
                    <div className="denuvo-game-image">
                      <img src={selectedGame.image} alt={selectedGame.name} />
                    </div>
                    <div className="denuvo-game-info">
                      <h3 className="denuvo-game-title">{selectedGame.name}</h3>
                      <p className="denuvo-game-appid">App ID: {selectedGame.appId}</p>
                    </div>
                  </div>

                  {/* Automated Bypass Activation */}
                  <div className="denuvo-automation-section">
                    <h4 className="denuvo-automation-title">Automated Bypass Activation</h4>
                    <p className="denuvo-automation-description">
                      Click "Install Bypass" to automatically process Bypass installation for this game.
                    </p>

                    {/* Progress Section - moved here */}
                    {showBypassProgress && (
                      <div className="bypass-progress-section">
                        <h3 className="progress-section-title">Activation Progress</h3>
                        <div className="activation-steps-container">
                          {/* Step 1: Initializing */}
                          <div className="activation-step">
                            <div className={`step-circle ${bypassProgress.progress >= 0 ? (bypassProgress.progress >= 10 ? 'completed' : 'active') : 'pending'}`}>
                              {bypassProgress.progress >= 10 ? <FiCheck size={16} /> : 
                               bypassProgress.progress >= 0 ? <div className="spinner-small"></div> : '1'}
                            </div>
                            <span className="step-label">Initializing</span>
                          </div>

                          {/* Step 2: Steam Detection */}
                          <div className="activation-step">
                            <div className={`step-circle ${bypassProgress.progress >= 10 ? (bypassProgress.progress >= 20 ? 'completed' : 'active') : 'pending'}`}>
                              {bypassProgress.progress >= 20 ? <FiCheck size={16} /> : 
                               bypassProgress.progress >= 10 ? <div className="spinner-small"></div> : '2'}
                            </div>
                            <span className="step-label">Steam Detection</span>
                          </div>

                          {/* Step 3: Game Validation */}
                          <div className="activation-step">
                            <div className={`step-circle ${bypassProgress.progress >= 20 ? (bypassProgress.progress >= 30 ? 'completed' : 'active') : 'pending'}`}>
                              {bypassProgress.progress >= 30 ? <FiCheck size={16} /> : 
                               bypassProgress.progress >= 20 ? <div className="spinner-small"></div> : '3'}
                            </div>
                            <span className="step-label">Game Validation</span>
                          </div>

                          {/* Step 4: Downloading Bypass */}
                          <div className="activation-step">
                            <div className={`step-circle ${bypassProgress.progress >= 30 ? (bypassProgress.progress >= 60 ? 'completed' : 'active') : 'pending'}`}>
                              {bypassProgress.progress >= 60 ? <FiCheck size={16} /> : 
                               bypassProgress.progress >= 30 ? <div className="spinner-small"></div> : '4'}
                            </div>
                            <div className="step-content">
                              <span className="step-label">Downloading Bypass</span>
                              {/* Progress Details for Download Step */}
                              {bypassProgress.progress >= 30 && bypassProgress.progress < 60 && bypassProgress.step && (
                                <div className="download-progress-details">
                                  <div className="download-progress-bar">
                                    <div 
                                      className="download-progress-fill" 
                                      style={{ width: `${Math.max(5, ((bypassProgress.progress - 30) / 30) * 100)}%` }}
                                    ></div>
                                  </div>
                                  <div className="download-info">
                                    <span className="download-status">{bypassProgress.step}</span>
                                    <div className="download-controls">
                                      <button 
                                        className={`download-control-btn pause ${downloadPaused ? 'paused' : ''}`}
                                        onClick={handlePauseDownload}
                                        title={downloadPaused ? "Resume Download" : "Pause Download"}
                                      >
                                        {downloadPaused ? '▶️' : '⏸️'}
                                      </button>
                                      <button 
                                        className="download-control-btn cancel"
                                        onClick={handleCancelDownload}
                                        title="Cancel Download"
                                      >
                                        ❌
                                      </button>
                                    </div>
                                  </div>
                                </div>
                              )}
                            </div>
                          </div>

                          {/* Step 5: Extracting Files */}
                          <div className="activation-step">
                            <div className={`step-circle ${bypassProgress.progress >= 60 ? (bypassProgress.progress >= 85 ? 'completed' : 'active') : 'pending'}`}>
                              {bypassProgress.progress >= 85 ? <FiCheck size={16} /> : 
                               bypassProgress.progress >= 60 ? <div className="spinner-small"></div> : '5'}
                            </div>
                            <span className="step-label">Extracting Files</span>
                          </div>

                          {/* Step 6: Installing Bypass */}
                          <div className="activation-step">
                            <div className={`step-circle ${bypassProgress.progress >= 85 ? (bypassProgress.progress >= 95 ? 'completed' : 'active') : 'pending'}`}>
                              {bypassProgress.progress >= 95 ? <FiCheck size={16} /> : 
                               bypassProgress.progress >= 85 ? <div className="spinner-small"></div> : '6'}
                            </div>
                            <span className="step-label">Installing Bypass</span>
                          </div>

                          {/* Step 7: Finalizing */}
                          <div className="activation-step">
                            <div className={`step-circle ${bypassProgress.progress >= 95 ? (bypassProgress.progress >= 100 ? 'completed' : 'active') : 'pending'}`}>
                              {bypassProgress.progress >= 100 ? <FiCheck size={16} /> : 
                               bypassProgress.progress >= 95 ? <div className="spinner-small"></div> : '7'}
                            </div>
                            <span className="step-label">Finalizing</span>
                          </div>
                        </div>
                      </div>
                    )}

                    {/* Status Items */}
                    <div className="denuvo-status-items">
                      <div className="denuvo-status-item">
                        <span className="denuvo-status-label">Game Installed:</span>
                        {gameInstallationStatus.isChecking ? (
                          <span className="denuvo-status-value validating">
                            <div className="spinner small"></div>
                            Checking...
                          </span>
                        ) : gameInstallationStatus.isInstalled ? (
                          <span className="denuvo-status-value success">
                            <FiCheck size={16} />
                            Yes
                          </span>
                        ) : (
                          <span className="denuvo-status-value error">
                            <FiX size={16} />
                            No
                          </span>
                        )}
                      </div>
                      <div className="denuvo-status-item">
                        <span className="denuvo-status-label">Bypass Downloaded:</span>
                        <span className="denuvo-status-value success">
                          <FiCheck size={16} />
                          Ready
                        </span>
                      </div>
                      <div className="denuvo-status-item">
                        <span className="denuvo-status-label">Game Path:</span>
                        <div className="denuvo-path-container">
                          <span className="denuvo-status-value path">
                            {gameInstallationStatus.gamePath || "Game not found"}
                            {gameInstallationStatus.isManualPath && (
                              <span className="manual-path-indicator"> (Manual)</span>
                            )}
                          </span>
                          <button 
                            className="browse-path-btn"
                            onClick={browseManualGamePath}
                            title="Browse for game folder"
                          >
                            <FiFolder size={14} />
                            Browse
                          </button>
                        </div>
                      </div>
                    </div>
                  </div>

                  {/* Activate Button */}
                  {!showBypassProgress && (
                    <button 
                      className={`denuvo-activate-btn-bottom ${(!gameInstallationStatus.isInstalled && !gameInstallationStatus.gamePath) || gameInstallationStatus.isChecking ? 'disabled' : ''}`}
                      onClick={handleBypassConfirm}
                      disabled={(!gameInstallationStatus.isInstalled && !gameInstallationStatus.gamePath) || gameInstallationStatus.isChecking}
                    >
                      <FiShield size={16} />
                      {gameInstallationStatus.isChecking ? 'Checking Game...' : 
                       (!gameInstallationStatus.isInstalled && !gameInstallationStatus.gamePath) ? 'Game Not Installed' : 
                       gameInstallationStatus.isManualPath ? 'Install Bypass (Manual Path)' :
                       'Install Bypass'}
                    </button>
                  )}
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
      </div>
    </div>
  );
}

export default Bypass;
