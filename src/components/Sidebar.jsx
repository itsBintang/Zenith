import React, { useState } from "react";
import { FiHome, FiBox, FiDownload, FiSettings, FiLogIn, FiRefreshCw } from "react-icons/fi";
import { invoke } from "@tauri-apps/api/tauri";
import MyLibrary from './MyLibrary'; // Assuming MyLibrary is in the same folder

function Sidebar({ active = "home", onNavigate, onGameSelect }) {
  const [isRestarting, setIsRestarting] = useState(false);
  const [notification, setNotification] = useState(null);

  const handleRestartSteam = async () => {
    if (isRestarting) return;
    
    setIsRestarting(true);
    try {
      const result = await invoke('restart_steam');
      setNotification({
        message: 'Steam has been restarted successfully',
        type: 'success'
      });
      
      // Auto close after 5 seconds
      setTimeout(() => {
        setNotification(null);
      }, 5000);
      
    } catch (error) {
      console.error('Failed to restart Steam:', error);
      setNotification({
        message: `Failed to restart Steam: ${error}`,
        type: 'error'
      });
      
      // Auto close after 5 seconds
      setTimeout(() => {
        setNotification(null);
      }, 5000);
    } finally {
      setIsRestarting(false);
    }
  };

  const closeNotification = () => {
    setNotification(null);
  };

  return (
    <aside className="ui-sidebar">
      <div className="ui-sidebar__section">
        <button className="ui-btn ui-btn--ghost ui-btn--lg">
          <FiLogIn size={18} />
          <span>Sign in</span>
        </button>
      </div>

      <nav className="ui-sidebar__nav">
        <a className={`ui-nav-item ${active === "home" ? "ui-nav-item--active" : ""}`} onClick={() => onNavigate && onNavigate("home")}>
          <FiHome size={18} />
          <span>Home</span>
        </a>
        <a className={`ui-nav-item ${active === "catalogue" ? "ui-nav-item--active" : ""}`} onClick={() => onNavigate && onNavigate("catalogue")}>
          <FiBox size={18} />
          <span>Catalogue</span>
        </a>
        <a className={`ui-nav-item ${active === "downloads" ? "ui-nav-item--active" : ""}`} onClick={() => onNavigate && onNavigate("downloads")}>
          <FiDownload size={18} />
          <span>Downloads</span>
        </a>
        <a className={`ui-nav-item ${active === "settings" ? "ui-nav-item--active" : ""}`} onClick={() => onNavigate && onNavigate("settings")}>
          <FiSettings size={18} />
          <span>Settings</span>
        </a>
      </nav>

      <MyLibrary onGameSelect={onGameSelect} />
      
      {/* Steam Control Section */}
      <div className="ui-sidebar__steam-control">
        <button 
          className={`ui-btn ui-btn--steam ${isRestarting ? 'restarting' : ''}`}
          onClick={handleRestartSteam}
          disabled={isRestarting}
        >
          <FiRefreshCw size={16} className={isRestarting ? 'spinning' : ''} />
          <span>{isRestarting ? 'Restarting...' : 'Restart Steam'}</span>
        </button>
      </div>

      {/* Toast Notification */}
      {notification && (
        <div className="toast-notification-overlay">
          <div className={`toast-notification ${notification.type}`}>
            <div className="toast-content">
              <div className="toast-icon">
                {notification.type === 'success' ? '✓' : '✗'}
              </div>
              <div className="toast-message">{notification.message}</div>
              <button className="toast-close" onClick={closeNotification}>
                ×
              </button>
            </div>
          </div>
        </div>
      )}
      
    </aside>
  );
}

export default Sidebar;


