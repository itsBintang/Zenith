import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { FiArrowLeft, FiCloud, FiDownload, FiPlay, FiSettings, FiCheck, FiX, FiTrash2 } from "react-icons/fi";
import { GameDetailSkeleton } from "./SkeletonLoader";
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

// Sidebar component for requirements only
function Sidebar({ detail, activeTab, setActiveTab }) {
  const formatRequirements = (reqHtml) => {
    if (!reqHtml) return { __html: "<p>No requirements specified</p>" };
    return { __html: reqHtml };
  };

  return (
    <aside className="content-sidebar">
      {/* Requirements Section */}
      <div className="sidebar-section">
        <h3 className="sidebar-section__title">Requirements</h3>
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
        <div 
          className="requirement__details"
          dangerouslySetInnerHTML={formatRequirements(
            activeTab === 'minimum' ? detail.pc_requirements?.minimum : detail.pc_requirements?.recommended
          )}
        />
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

function GameDetail({ appId, onBack }) {
  const [detail, setDetail] = useState(null);
  const [activeTab, setActiveTab] = useState("minimum");
  const [gameColor, setGameColor] = useState("#1a1a1a");
  const [isDownloading, setIsDownloading] = useState(false);
  const [isInLibrary, setIsInLibrary] = useState(false);
  const [notification, setNotification] = useState(null);

  useEffect(() => {
    let mounted = true;
    invoke("get_game_details", { appId }).then((d) => { 
      if (mounted) {
        setDetail(d);
        // Simple color extraction - in real implementation, you'd extract from the hero image
        setGameColor("#2a2a3a");
        // Check if game is already in library
        checkIfInLibrary(appId);
      }
    });
    return () => { mounted = false; };
  }, [appId]);

  const checkIfInLibrary = async (appId) => {
    try {
      // Use a more specific backend call instead of fetching all games
      const isInLibrary = await invoke("check_game_in_library", { appId });
      setIsInLibrary(isInLibrary);
    } catch (error) {
      console.error("Error checking library:", error);
      // Fallback to the full library check if the specific call fails
      try {
        const libraryGames = await invoke("get_library_games");
        const gameInLibrary = libraryGames.some(game => game.app_id === appId);
        setIsInLibrary(gameInLibrary);
      } catch (fallbackError) {
        console.error("Fallback check also failed:", fallbackError);
      }
    }
  };

  const handleAddToLibrary = async () => {
    if (!detail || isDownloading) return;

    setIsDownloading(true);
    try {
      const result = await invoke("download_game", { 
        appId: detail.app_id, 
        gameName: detail.name 
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

  if (!detail) {
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
        {/* Hero Section */}
        <div className="game-details__hero">
          <img
            src={detail.header_image || detail.banner_image}
            className="game-details__hero-image"
            alt={detail.name}
          />
          <div
            className="game-details__hero-backdrop"
            style={{ backgroundColor: gameColor }}
          />
          
          <div className="game-details__hero-logo-backdrop">
            <div className="game-details__hero-content">
              <button onClick={onBack} className="game-details__back-button">
                <FiArrowLeft size={24} />
              </button>
              
              <h1 className="game-details__game-logo">{detail.name}</h1>
              
              <button className="game-details__cloud-sync-button">
                <FiCloud />
                Cloud save
              </button>
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
            {/* Header info */}
            <div className="description-header">
              <div className="description-header__row">
                <span>Released on {detail.release_date || "N/A"}</span>
                <span className="description-header__separator">•</span>
                <span>Published by {detail.publisher || "Unknown"}</span>
              </div>
            </div>

            {/* Gallery */}
            <GallerySlider screenshots={detail.screenshots} />

            {/* Description */}
            <div 
              className="game-details__description"
              dangerouslySetInnerHTML={{ __html: detail.detailed_description || "<p>No description available</p>" }}
            />
          </div>

          {/* Sidebar */}
          <Sidebar detail={detail} activeTab={activeTab} setActiveTab={setActiveTab} />
        </div>
      </section>
    </div>
  );
}

export default GameDetail;