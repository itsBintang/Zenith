import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FiArrowLeft, FiCloud, FiDownload, FiPlay, FiSettings, FiCheck, FiX, FiTrash2, FiPackage, FiRefreshCw } from "react-icons/fi";
import { GameDetailSkeleton } from "./SkeletonLoader";
import DlcManager from "./DlcManager";
import "../styles/GameDetail.css";
import { isTauri } from '@tauri-apps/api/core';

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
function HeroPanel({ detail, onAddToLibrary, onRemoveFromLibrary, isDownloading, isInLibrary, bypassInstalled, onLaunchBypassGame, isLaunching }) {
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
          <>
            {bypassInstalled && (
              <button 
                className="hero-button hero-button--play" 
                onClick={onLaunchBypassGame}
                disabled={isLaunching || isDownloading}
              >
                <FiPlay /> 
                {isLaunching ? "Launching..." : "Play"}
              </button>
            )}
            <button 
              className="hero-button hero-button--danger" 
              onClick={onRemoveFromLibrary}
              disabled={isDownloading}
            >
              <FiTrash2 /> 
              {isDownloading ? "Removing..." : "Remove from library"}
            </button>
          </>
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
  
 
  // Update related states
  const [isUpdating, setIsUpdating] = useState(false);

  // Bypass related states
  const [bypassInstalled, setBypassInstalled] = useState(false);
  const [gameExecutables, setGameExecutables] = useState([]);
  const [isLaunching, setIsLaunching] = useState(false);

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
          checkBypassStatus(appId);
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

  const checkBypassStatus = async (appId) => {
    try {
      const isRunningInTauri = await isTauri();
      if (!isRunningInTauri) return;

      const isInstalled = await invoke("check_bypass_installed_command", { appId });
      setBypassInstalled(isInstalled);
      
      if (isInstalled) {
        // Load game executables when bypass is installed
        await loadGameExecutables(appId);
      }
    } catch (error) {
      console.error("Error checking bypass status:", error);
      setBypassInstalled(false);
    }
  };

  const loadGameExecutables = async (appId) => {
    try {
      // Get game installation path first
      const gameInfo = await invoke("get_game_installation_info", { appId });
      if (gameInfo && gameInfo.install_path) {
        const bypassNotes = await invoke("get_bypass_notes", { 
          gamePath: gameInfo.install_path
        });
        setGameExecutables(bypassNotes.exe_list || []);
      }
    } catch (error) {
      console.error("Error loading game executables:", error);
      setGameExecutables([]);
    }
  };

  const handleLaunchBypassGame = async () => {
    if (!detail || !bypassInstalled || gameExecutables.length === 0 || isLaunching) return;

    setIsLaunching(true);
    try {
      // Use the first/recommended executable or show selection if multiple
      const selectedExecutable = gameExecutables[0]; // For now, use first one
      
      const result = await invoke("confirm_and_launch_game", { 
        executablePath: selectedExecutable.path,
        gameName: detail.name
      });
      
      setNotification({
        message: `${detail.name} launched successfully with bypass!`,
        type: 'success'
      });
      
      console.log("Launch result:", result);
    } catch (error) {
      console.error("Launch game error:", error);
      setNotification({
        message: `Failed to launch ${detail.name}: ${error}`,
        type: 'error'
      });
    } finally {
      setIsLaunching(false);
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

  const handleUpdateGame = async () => {
    if (!detail || isUpdating) return;
  
    setIsUpdating(true);
    setNotification({
      message: `Searching for ${detail.name} updates...`,
      type: 'info' // Or a different type for ongoing processes
    });
  
    try {
      const result = await invoke("update_game_files", { 
        appId: detail.app_id,
        gameName: detail.name 
      });
      
      setNotification({
        message: result, // Success or error message from backend
        type: 'success'
      });
    } catch (error) {
      console.error("Update error:", error);
      setNotification({
        message: `Update failed: ${error}`,
        type: 'error'
      });
    } finally {
      setIsUpdating(false);
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
              {isInLibrary && (
                <button 
                  className={`game-details__update-button ${isUpdating ? 'updating' : ''}`}
                  onClick={handleUpdateGame}
                  disabled={isUpdating}
                >
                  {isUpdating ? (
                    <>
                      <div className="spinner"></div>
                      Updating...
                    </>
                  ) : (
                    <>
                      <FiRefreshCw />
                      Update
                    </>
                  )}
                </button>
              )}
              
              
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
          bypassInstalled={bypassInstalled}
          onLaunchBypassGame={handleLaunchBypassGame}
          isLaunching={isLaunching}
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



    </div>
  );
}

export default GameDetail;