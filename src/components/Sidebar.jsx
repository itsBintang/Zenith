import React, { useState, useEffect } from "react";
import { NavLink, useNavigate } from "react-router-dom";
import { FiHome, FiBox, FiSettings, FiRefreshCw, FiShield } from "react-icons/fi";
import { invoke } from "@tauri-apps/api/core";
import logoImage from "../../logo.jpg";
import MyLibrary from './MyLibrary'; // Assuming MyLibrary is in the same folder

function Sidebar({ libraryState, onRefreshLibrary, onUpdateFilter, refreshProfileTrigger }) {
  const [isRestarting, setIsRestarting] = useState(false);
  const [notification, setNotification] = useState(null);
  const [profile, setProfile] = useState(null);
  const [avatarImage, setAvatarImage] = useState(null);
  const navigate = useNavigate();

  // Load profile data on component mount
  useEffect(() => {
    loadProfile();
  }, []);

  // Reload profile when refreshProfileTrigger changes
  useEffect(() => {
    if (refreshProfileTrigger) {
      loadProfile();
    }
  }, [refreshProfileTrigger]);

  const loadProfile = async () => {
    try {
      // Load profile data
      const profileData = await invoke('get_user_profile');
      setProfile(profileData);
      
      // Load avatar image as base64
      try {
        const avatarBase64 = await invoke('get_profile_image_base64', { imageType: 'avatar' });
        setAvatarImage(avatarBase64);
      } catch (imageError) {
        console.error('Failed to load avatar image:', imageError);
        setAvatarImage(null);
      }
    } catch (error) {
      console.error('Failed to load profile:', error);
      // Set default profile if loading fails
      setProfile({ name: 'User' });
    }
  };

  const handleProfileClick = () => {
    navigate('/profile');
  };

  const handleGameSelect = (appId) => {
    navigate(`/game/${appId}`);
  };

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
      {/* Profile Section */}
      <div className="ui-sidebar__section">
        <div className="ui-profile" onClick={handleProfileClick}>
          <img 
            src={avatarImage || logoImage} 
            alt={profile?.name || 'User'} 
            className="ui-profile__avatar" 
          />
          <span className="ui-profile__name">{profile?.name || 'User'}</span>
        </div>
      </div>

      {/* Scrollable Area */}
      <div className="ui-sidebar__scroll-area">
        {/* Main Navigation */}
        <nav className="ui-sidebar__nav">
          <NavLink to="/" className={({ isActive }) => `ui-nav-item ${isActive ? "ui-nav-item--active" : ""}`} end>
            <FiHome size={18} />
            <span>Home</span>
          </NavLink>
          <NavLink to="/catalogue" className={({ isActive }) => `ui-nav-item ${isActive ? "ui-nav-item--active" : ""}`}>
            <FiBox size={18} />
            <span>Catalogue</span>
          </NavLink>
          <NavLink to="/bypass" className={({ isActive }) => `ui-nav-item ${isActive ? "ui-nav-item--active" : ""}`}>
            <FiShield size={18} />
            <span>Bypass</span>
          </NavLink>
          <NavLink to="/settings" className={({ isActive }) => `ui-nav-item ${isActive ? "ui-nav-item--active" : ""}`}>
            <FiSettings size={18} />
            <span>Settings</span>
          </NavLink>
        </nav>

        {/* My Library Section */}
        <div className="ui-sidebar__library-container">
          <MyLibrary 
            onGameSelect={handleGameSelect}
            libraryState={libraryState}
            onRefreshLibrary={onRefreshLibrary}
            onUpdateFilter={onUpdateFilter}
          />
        </div>
      </div>
      
      {/* Steam Control Section - Always at bottom */}
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


