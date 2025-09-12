import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FiArrowLeft, FiCloud, FiDownload, FiPlay, FiSettings, FiCheck, FiX, FiTrash2, FiPackage, FiShield } from "react-icons/fi";
import { GameDetailSkeleton } from "./SkeletonLoader";
import DlcManager from "./DlcManager";
import "../styles/GameDetail.css";

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

// HeroPanel component for actions and info
function HeroPanel({ detail, onAddToLibrary, onRemoveFromLibrary, isDownloading, isInLibrary }) {
  const formatDate = (dateStr) => {
    if (!dateStr) return "N/A";
    const date = new Date(dateStr);
    return date.toLocaleDateString();
  };

  return (
    <div className="hero-panel">
      <div className="hero-panel__content">
        <p>Released on {formatDate(detail.release_date)}</p>
        <p>Published by {detail.publisher || "Unknown"}</p>
      </div>
      <div className="hero-panel__actions">
        {!isInLibrary ? (
          <button 
            className="hero-button hero-button--primary" 
            onClick={onAddToLibrary}
            disabled={isDownloading}
          >
            <FiDownload /> 
            {isDownloading ? "Installing..." : "Add to library"}
          </button>
        ) : (
          <button 
            className="hero-button hero-button--danger" 
            onClick={onRemoveFromLibrary}
            disabled={isDownloading}
          >
            <FiTrash2 /> 
            {isDownloading ? "Removing..." : "Remove from library"}
          </button>
        )}
      </div>
    </div>
  );
}

// Sidebar component for requirements and DRM
function Sidebar({ detail, activeTab, setActiveTab, drmData, loadingDrm }) {
  const formatRequirements = (reqHtml) => {
    if (!reqHtml) return { __html: "<p>No requirements specified</p>" };
    
    // Remove the "Minimum:" or "Recommended:" prefix if it exists
    const cleanedHtml = reqHtml.replace(/^(<strong>)?(Minimum|Recommended):?(<\/strong>)?(<br>)?/i, '').trim();
    return { __html: cleanedHtml };
  };

  return (
    <aside className="content-sidebar">
      {/* DRM Section */}
      {(drmData || loadingDrm) && (
        <div className="sidebar-section">
          <h3 className="sidebar-section__title">DRM Notice</h3>
          <div className="drm__content">
            {loadingDrm ? (
              <p className="drm__loading">Loading DRM information...</p>
            ) : drmData ? (
              <div 
                className="drm__notice"
                dangerouslySetInnerHTML={{ __html: drmData }}
              />
            ) : (
              <p className="drm__none">No DRM information available</p>
            )}
          </div>
        </div>
      )}

      {/* Requirements Section */}
      <div className="sidebar-section">
        <h3 className="sidebar-section__title">System requirements</h3>
        <div className="requirement__button-container">
          <button
            className={`requirement__button ${activeTab === 'minimum' ? 'active' : ''}`}
            onClick={() => setActiveTab('minimum')}
          >
            Minimum
          </button>
          <button
            className={`requirement__button ${activeTab === 'recommended' ? 'active' : ''}`}
            onClick={() => setActiveTab('recommended')}
          >
            Recommended
          </button>
        </div>
        <div className="requirement__content">
          <h4 className="requirement__type-title">{activeTab === 'minimum' ? 'Minimum:' : 'Recommended:'}</h4>
          <div 
            className="requirement__details"
            dangerouslySetInnerHTML={formatRequirements(
              activeTab === 'minimum' ? detail.pc_requirements?.minimum : detail.pc_requirements?.recommended
            )}
          />
        </div>
      </div>
    </aside>
  );
}

// Gallery slider component
function GallerySlider({ screenshots }) {
  const [currentIndex, setCurrentIndex] = useState(0);

  if (!screenshots || screenshots.length === 0) return null;

  return (
    <div className="gallery-slider">
      <div className="gallery-slider__main">
        <img 
          src={screenshots[currentIndex]} 
          alt={`Screenshot ${currentIndex + 1}`}
          className="gallery-slider__image"
        />
      </div>
      <div className="gallery-slider__thumbnails">
        {screenshots.map((screenshot, index) => (
          <img
            key={index}
            src={screenshot}
            alt={`Thumbnail ${index + 1}`}
            className={`gallery-slider__thumbnail ${index === currentIndex ? 'active' : ''}`}
            onClick={() => setCurrentIndex(index)}
          />
        ))}
      </div>
    </div>
  );
}

function GameDetail({ appId, onBack, showBackButton = true }) {
  const [detail, setDetail] = useState(null);
  const [isLoading, setIsLoading] = useState(true);
  const [activeTab, setActiveTab] = useState("minimum");
  const [gameColor, setGameColor] = useState("#1a1a1a");
  const [isDownloading, setIsDownloading] = useState(false);
  const [isInLibrary, setIsInLibrary] = useState(false);
  const [notification, setNotification] = useState(null);
  const [showDlcManager, setShowDlcManager] = useState(false);
  const [drmData, setDrmData] = useState(null);
  const [loadingDrm, setLoadingDrm] = useState(false);
  
  // Bypass related states
  const [bypassStatus, setBypassStatus] = useState({
    validating: false,
    installing: false,
    installed: false
  });
  const [showBypassProgress, setShowBypassProgress] = useState(false);
  const [bypassProgress, setBypassProgress] = useState({ step: '', progress: 0 });
  const [showBypassConfirmation, setShowBypassConfirmation] = useState(false);
  const [showLaunchPopup, setShowLaunchPopup] = useState(false);
  const [gameExecutablePath, setGameExecutablePath] = useState(null);
  const [gameExecutables, setGameExecutables] = useState([]);
  const [loadingExecutables, setLoadingExecutables] = useState(false);

  useEffect(() => {
    let mounted = true;
    let hasRequested = false;
    
    const loadGameDetails = async () => {
      // Prevent double request in React.StrictMode
      if (hasRequested) return;
      hasRequested = true;
      
      try {
        setIsLoading(true);
        const d = await invoke("get_game_details", { appId });
        if (mounted) {
          setDetail(d);
          setGameColor("#2a2a3a");
          setIsLoading(false);
          checkIfInLibrary(appId);
          // Check if bypass is already installed
          if (typeof window !== 'undefined' && window.__TAURI__) {
            checkIfBypassInstalled(appId);
          }
          // DRM data sudah ada di game detail response
          if (d.drm_notice) {
            setDrmData(d.drm_notice);
          } else {
            setDrmData("No DRM information available");
          }
          setLoadingDrm(false);
        }
      } catch (error) {
        console.error("Failed to load game details:", error);
        if (mounted) {
          setDetail(null);
          setIsLoading(false);
        }
      }
    };
    
    loadGameDetails();
    
    return () => { 
      mounted = false; 
    };
  }, [appId]);

  // Bypass progress listener
  useEffect(() => {
    let unlistenFn = null;
    
    const setupListener = async () => {
      try {
        // Check if we're in Tauri environment
        if (typeof window !== 'undefined' && window.__TAURI__) {
          unlistenFn = await listen('bypass_progress', (event) => {
            const progress = event.payload;
            if (progress.app_id === appId) {
              setBypassProgress(progress);
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
  }, [appId]);

  const checkIfInLibrary = async (appId) => {
    try {
      // Use a more specific backend call instead of fetching all games
      const isInLibrary = await invoke("check_game_in_library", { appId });
      setIsInLibrary(isInLibrary);
    } catch (error) {
      console.error("Error checking library:", error);
      // Don't fallback to fetching all games - it's too expensive
      setIsInLibrary(false);
    }
  };

  const checkIfBypassInstalled = async (appId) => {
    try {
      const isInstalled = await invoke("check_bypass_installed_command", { appId });
      setBypassStatus(prev => ({
        ...prev,
        installed: isInstalled
      }));
    } catch (error) {
      console.error("Error checking bypass installation:", error);
      setBypassStatus(prev => ({
        ...prev,
        installed: false
      }));
    }
  };

  const handleBypassClick = async () => {
    if (!detail || bypassStatus.validating || bypassStatus.installing) return;
    
    // Check if we're in Tauri environment
    if (typeof window === 'undefined' || !window.__TAURI__) {
      setNotification({
        message: "Bypass is only available in desktop app",
        type: 'error'
      });
      return;
    }

    // Start validation
    setBypassStatus(prev => ({ ...prev, validating: true }));

    try {
      console.log("🔍 Validating bypass availability for:", detail.app_id);
      
      const availability = await invoke("check_bypass_availability", { 
        appId: detail.app_id 
      });

      if (availability.available) {
        console.log("✅ Bypass available - showing confirmation popup");
        setBypassStatus(prev => ({ 
          ...prev, 
          validating: false,
          installed: availability.installed 
        }));
        setShowBypassConfirmation(true);
      } else {
        console.log("❌ Bypass not available");
        setNotification({
          message: `Bypass not available for ${detail.name}`,
          type: 'error'
        });
        setBypassStatus(prev => ({ ...prev, validating: false }));
      }
    } catch (error) {
      console.error("Bypass validation error:", error);
      setNotification({
        message: `Failed to check bypass availability: ${error}`,
        type: 'error'
      });
      setBypassStatus(prev => ({ ...prev, validating: false }));
    }
  };

  const handleBypassConfirm = async () => {
    setShowBypassConfirmation(false);
    
    const isReinstall = bypassStatus.installed;
    const actionMessage = isReinstall ? "Reinstalling bypass..." : "Installing bypass...";
    const successMessage = isReinstall ? "Bypass reinstalled successfully!" : "Bypass installed successfully!";

    setBypassStatus(prev => ({ ...prev, installing: true }));
    setShowBypassProgress(true);
    setBypassProgress({ step: actionMessage, progress: 0 });

    try {
      const result = await invoke("install_bypass", { 
        appId: detail.app_id 
      });

      if (result && result.success) {
        setNotification({
          message: successMessage,
          type: 'success'
        });
        
        setBypassStatus({
          validating: false,
          installing: false,
          installed: true
        });

        // Show launch popup if game executable found
        if (result.should_launch && result.game_executable_path) {
          setGameExecutablePath(result.game_executable_path);
          loadGameExecutables(result.game_executable_path);
          setShowLaunchPopup(true);
        }
      } else {
        setNotification({
          message: `Bypass ${isReinstall ? 'reinstallation' : 'installation'} failed: ${result?.message || 'Unknown error'}`,
          type: 'error'
        });
        setBypassStatus(prev => ({ ...prev, installing: false }));
      }
    } catch (error) {
      console.error("Bypass installation error:", error);
      setNotification({
        message: `Failed to ${isReinstall ? 'reinstall' : 'install'} bypass: ${error}`,
        type: 'error'
      });
      setBypassStatus(prev => ({ ...prev, installing: false }));
    } finally {
      setShowBypassProgress(false);
    }
  };

  const loadGameExecutables = async (gamePath) => {
    setLoadingExecutables(true);
    try {
      console.log("🔍 Loading executables from:", gamePath);
      const executables = await invoke("get_game_executables", { 
        gamePath: gamePath
      });
      
      console.log("📄 Found executables:", executables);
      setGameExecutables(executables);
    } catch (error) {
      console.error("Error loading executables:", error);
      setGameExecutables([]);
    } finally {
      setLoadingExecutables(false);
    }
  };

  const handleLaunchSelectedExecutable = async (executablePath, executableName) => {
    if (!detail) return;
    
    try {
      console.log("🎮 User selected executable:", executableName);
      console.log("📁 Path:", executablePath);
      
      const result = await invoke("confirm_and_launch_game", { 
        executablePath: executablePath,
        gameName: detail.name
      });
      
      setNotification({
        message: `${executableName} launched successfully! Game is starting with bypass enabled.`,
        type: 'success'
      });
      setShowLaunchPopup(false);
      
      console.log("✅ Launch result:", result);
    } catch (error) {
      console.error("Launch game error:", error);
      setNotification({
        message: `Failed to launch ${executableName}: ${error}`,
        type: 'error'
      });
    }
  };


  const handleAddToLibrary = async () => {
    if (!detail || isDownloading) return;

    setIsDownloading(true);
    try {
      const useTempZip = JSON.parse(localStorage.getItem('zenith.useTempZip') ?? 'true');
      const downloadFolder = localStorage.getItem('zenith.downloadFolder') || null;
      const result = await invoke("download_game", { 
        appId: detail.app_id, 
        gameName: detail.name,
        saveZip: !useTempZip,
        saveDir: downloadFolder
      });
      
      console.log("Download result:", result); // Debug log
      
      if (result && result.success) {
        setNotification({
          message: `Successfully installed ${detail.name}`,
          type: 'success'
        });
        setIsInLibrary(true);
      } else {
        setNotification({
          message: `Installation failed: ${result?.message || 'Unknown error'}`,
          type: 'error'
        });
      }
    } catch (error) {
      console.error("Download error:", error);
      setNotification({
        message: `Failed to install ${detail.name}: ${error}`,
        type: 'error'
      });
    } finally {
      setIsDownloading(false);
    }
  };

  const handleRemoveFromLibrary = async () => {
    if (!detail || isDownloading) return;

    setIsDownloading(true);
    try {
      const result = await invoke("remove_game", { 
        appId: detail.app_id
      });
      
      console.log("Remove result:", result); // Debug log
      
      if (result && result.success) {
        setNotification({
          message: `Successfully removed ${detail.name} from library`,
          type: 'success'
        });
        setIsInLibrary(false);
      } else {
        setNotification({
          message: `Failed to remove game: ${result?.message || 'Unknown error'}`,
          type: 'error'
        });
      }
    } catch (error) {
      console.error("Remove error:", error);
      setNotification({
        message: `Failed to remove ${detail.name}: ${error}`,
        type: 'error'
      });
    } finally {
      setIsDownloading(false);
    }
  };

  const closeNotification = () => {
    setNotification(null);
  };

  const handleManageDlcs = () => {
    setShowDlcManager(true);
  };

  const closeDlcManager = () => {
    setShowDlcManager(false);
  };

  const showNotificationFromDlc = (message, type) => {
    setNotification({ message, type });
  };

  if (isLoading || !detail) {
    return <GameDetailSkeleton />;
  }

  return (
    <div className="game-details__wrapper">
      {notification && (
        <ToastNotification
          message={notification.message}
          type={notification.type}
          onClose={closeNotification}
        />
      )}
      <section className="game-details__container">
        {/* Navigation Bar */}
        <div className="game-details__navbar">
          {showBackButton && (
            <button onClick={onBack} className="game-details__back-button">
              <FiArrowLeft size={24} />
            </button>
          )}
          <h1 className="game-details__title">{detail.name}</h1>
        </div>

        {/* Hero Section */}
        <div className="game-details__hero">
          <img
            src={`https://cdn.akamai.steamstatic.com/steam/apps/${detail.app_id}/library_hero.jpg`}
            className="game-details__hero-image"
            alt={detail.name}
            onError={(e) => {
              // Fallback to original image if CDN fails
              e.target.src = detail.header_image || detail.banner_image;
            }}
          />
          
          <div className="game-details__hero-controls">
            <div className="game-details__action-buttons">
              <button className="game-details__cloud-sync-button">
                <FiCloud />
                Cloud save
              </button>
              
              {/* Universal Bypass Button */}
              <button 
                className={`game-details__bypass-button ${
                  bypassStatus.installed ? 'installed' : ''
                } ${bypassStatus.installing ? 'installing' : ''} ${
                  bypassStatus.validating ? 'validating' : ''
                }`}
                onClick={handleBypassClick}
                disabled={bypassStatus.installing || bypassStatus.validating}
              >
                {bypassStatus.installing ? (
                  <>
                    <div className="spinner"></div>
                    Installing Bypass...
                  </>
                ) : bypassStatus.validating ? (
                  <>
                    <div className="spinner"></div>
                    Checking Bypass...
                  </>
                ) : bypassStatus.installed ? (
                  <>
                    <FiShield />
                    REINSTALL BYPASS
                  </>
                ) : (
                  <>
                    <FiShield />
                    BYPASS
                  </>
                )}
              </button>
              
              {isInLibrary && (
                <button 
                  className="game-details__dlc-button"
                  onClick={handleManageDlcs}
                  disabled={isDownloading}
                >
                  <FiPackage />
                  DLC Unlocker
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Hero Panel */}
        <HeroPanel 
          detail={detail} 
          onAddToLibrary={handleAddToLibrary}
          onRemoveFromLibrary={handleRemoveFromLibrary}
          isDownloading={isDownloading}
          isInLibrary={isInLibrary}
        />

        {/* Main Content */}
        <div className="game-details__description-container">
          <div className="game-details__description-content">

            {/* Gallery */}
            <GallerySlider screenshots={detail.screenshots} />

            {/* Description */}
            <div 
              className="game-details__description"
              dangerouslySetInnerHTML={{ __html: detail.detailed_description || "<p>No description available</p>" }}
            />
          </div>

          {/* Sidebar */}
          <Sidebar detail={detail} activeTab={activeTab} setActiveTab={setActiveTab} drmData={drmData} loadingDrm={loadingDrm} />
        </div>
      </section>

      {/* DLC Manager Modal */}
      {showDlcManager && (
        <DlcManager 
          game={detail}
          onClose={closeDlcManager}
          showNotification={showNotificationFromDlc}
        />
      )}

      {/* Bypass Confirmation Modal */}
      {showBypassConfirmation && (
        <div className="modal-overlay">
          <div className="bypass-confirmation-modal">
            <div className="confirmation-content">
              <FiShield className="bypass-icon" />
              <h3>🎯 Bypass Available!</h3>
              <p className="confirmation-message">
                Bypass tersedia untuk <strong>{detail?.name}</strong>. 
                {bypassStatus.installed ? (
                  <>
                    <br />
                    <span className="reinstall-note">
                      Bypass sudah terinstall sebelumnya. Mau reinstall bypass?
                    </span>
                  </>
                ) : (
                  <>
                    <br />
                    Mau install bypass untuk game ini?
                  </>
                )}
              </p>
              
              <div className="confirmation-warning">
                <p>⚠️ <strong>Penting:</strong> Bypass akan replace file game. Pastikan game sudah diinstall dengan benar.</p>
              </div>
              
              <div className="confirmation-actions">
                <button 
                  className="confirmation-button primary"
                  onClick={handleBypassConfirm}
                >
                  <FiShield />
                  {bypassStatus.installed ? 'Yes, Reinstall' : 'Yes, Install'}
                </button>
                <button 
                  className="confirmation-button secondary"
                  onClick={() => setShowBypassConfirmation(false)}
                >
                  Cancel
                </button>
              </div>
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
              <FiPlay className="launch-icon" />
              <h3>🎉 Bypass Berhasil Diinstall!</h3>
              
              <div className="launch-popup-info">
                <div className="info-section">
                  <h4>📋 Penting untuk Diperhatikan:</h4>
                  <ul className="info-list">
                    <li>✅ <strong>Bypass hanya berhasil</strong> jika kamu main <strong>PERTAMA KALI</strong> lewat launcher ini</li>
                    <li>🚫 <strong>JANGAN</strong> buka game lewat Steam atau shortcut lain</li>
                    <li>🎯 <strong>WAJIB</strong> launch dari tombol "Yes, Launch Game" di bawah</li>
                    <li>🔄 Kalau udah pernah buka lewat Steam, bypass mungkin gak jalan</li>
                  </ul>
                </div>
                
                <div className="warning-section">
                  <p className="warning-text">
                    ⚠️ <strong>Ingat:</strong> Bypass cuma work kalau game di-launch lewat path yang benar pertama kali. 
                    Kalau kamu buka lewat Steam dulu, kemungkinan bypass gak akan aktif.
                  </p>
                </div>
              </div>

              <div className="launch-popup-question">
                <h4>🎮 Pilih executable untuk launch game:</h4>
                <p className="question-subtitle">
                  Pilih file .exe yang paling besar (biasanya itu main game). Bypass cuma work kalau launch dari sini!
                </p>
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
                      className={`executable-item ${index === 0 ? 'recommended' : ''}`}
                      onClick={() => handleLaunchSelectedExecutable(exe.path, exe.name)}
                    >
                      <div className="executable-info">
                        <div className="executable-name">
                          {index === 0 && <span className="recommended-badge">🏆 RECOMMENDED</span>}
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
                  onClick={() => setShowLaunchPopup(false)}
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

export default GameDetail;