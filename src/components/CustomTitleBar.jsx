import React, { useState, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import ChangelogModal from './ChangelogModal';
import './CustomTitleBar.css';

const CustomTitleBar = ({ theme = 'app-theme' }) => {
  const [isMaximized, setIsMaximized] = useState(false);
  const [appWindow, setAppWindow] = useState(null);
  const [isMenuOpen, setIsMenuOpen] = useState(false);
  const [showChangelogModal, setShowChangelogModal] = useState(false);
  
  // Update-related state
  const [updateStatus, setUpdateStatus] = useState('idle'); // idle, checking, available, downloading, error
  const [updateMessage, setUpdateMessage] = useState('');
  const [showUpdateDialog, setShowUpdateDialog] = useState(false);

  useEffect(() => {
    const initWindow = async () => {
      const window = getCurrentWindow();
      setAppWindow(window);
      
      // Check if window is maximized on startup
      const maximized = await window.isMaximized();
      setIsMaximized(maximized);

      // Listen for window state changes
      const unlistenResize = await window.onResized(() => {
        checkMaximizedState(window);
      });

      return () => {
        unlistenResize();
      };
    };

    initWindow();
  }, []);

  const checkMaximizedState = async (window) => {
    try {
      const maximized = await window.isMaximized();
      setIsMaximized(maximized);
    } catch (error) {
      console.error('Error checking maximized state:', error);
    }
  };

  const handleMinimize = async () => {
    try {
      await appWindow?.minimize();
    } catch (error) {
      console.error('Error minimizing window:', error);
    }
  };

  const handleMaximize = async () => {
    try {
      await appWindow?.toggleMaximize();
    } catch (error) {
      console.error('Error toggling maximize:', error);
    }
  };

  const handleClose = async () => {
    try {
      await appWindow?.close();
    } catch (error) {
      console.error('Error closing window:', error);
    }
  };

  const toggleMenu = () => {
    setIsMenuOpen(!isMenuOpen);
  };

  const checkForUpdates = async () => {
    setUpdateStatus('checking');
    setUpdateMessage('');
    
    try {
      const result = await invoke('check_for_updates');
      
      // Check if result is valid and not null
      if (result && typeof result === 'string') {
        if (result.includes('Update available')) {
          setUpdateStatus('available');
          setUpdateMessage(result);
          setShowUpdateDialog(true);
        } else {
          setUpdateStatus('idle');
          setUpdateMessage(result || 'No updates available');
          // Show "no updates" message briefly
          setTimeout(() => setUpdateMessage(''), 3000);
        }
      } else {
        // Handle null or invalid result
        setUpdateStatus('idle');
        setUpdateMessage('No updates available');
        setTimeout(() => setUpdateMessage(''), 3000);
      }
    } catch (err) {
      setUpdateStatus('error');
      setUpdateMessage(`Update check failed: ${err}`);
      console.error('Update check failed:', err);
      setTimeout(() => {
        setUpdateMessage('');
        setUpdateStatus('idle');
      }, 5000);
    }
  };

  const installUpdate = async () => {
    setUpdateStatus('downloading');
    setUpdateMessage('');
    
    try {
      const result = await invoke('install_update');
      setUpdateStatus('installed');
      setUpdateMessage(result || 'Update installed successfully');
      
      // Show restart prompt
      setTimeout(() => {
        if (window.confirm('Update installed successfully! Would you like to restart the application now?')) {
          window.location.reload();
        }
      }, 1000);
      
    } catch (err) {
      setUpdateStatus('error');
      const errorMessage = err ? err.toString() : 'Unknown error occurred';
      let userFriendlyMessage = errorMessage;
      
      if (errorMessage.includes('Failed to install update')) {
        userFriendlyMessage = 'Installation failed. Please try again or download manually from our website.';
      } else if (errorMessage.includes('network')) {
        userFriendlyMessage = 'Download interrupted. Please check your internet connection and try again.';
      }
      
      setUpdateMessage(userFriendlyMessage);
      console.error('Update installation failed:', err);
      
      setTimeout(() => {
        setUpdateMessage('');
        setUpdateStatus('idle');
      }, 5000);
    }
  };

  const handleMenuItemClick = (action) => {
    switch (action) {
      case 'changelog':
        setIsMenuOpen(false);
        setShowChangelogModal(true);
        break;
      case 'check-updates':
        checkForUpdates();
        break;
      default:
        break;
    }
    setIsMenuOpen(false);
  };

  // Close menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (event) => {
      if (isMenuOpen && !event.target.closest('.titlebar-menu')) {
        setIsMenuOpen(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isMenuOpen]);

  return (
    <div className={`custom-titlebar ${theme}`}>
      <div className="titlebar-content" data-tauri-drag-region>
        <div className="titlebar-left">
          <div className="titlebar-icon">
            <img src="/logo.jpg" alt="Zenith" className="app-icon" />
          </div>
          <div className="titlebar-title">
            Zenith
          </div>
        </div>
        
        <div className="titlebar-spacer"></div>
        
        <div className="titlebar-controls">
          <div className="titlebar-menu">
            <button 
              className="menu-button"
              onClick={toggleMenu}
              title="Menu"
            >
              <svg width="12" height="12" viewBox="0 0 12 12">
                <circle cx="6" cy="2" r="1" fill="currentColor"/>
                <circle cx="6" cy="6" r="1" fill="currentColor"/>
                <circle cx="6" cy="10" r="1" fill="currentColor"/>
              </svg>
            </button>
            
            {isMenuOpen && (
              <div className="menu-dropdown">
                <div 
                  className="menu-item"
                  onClick={() => handleMenuItemClick('changelog')}
                >
                  <svg width="14" height="14" viewBox="0 0 14 14" className="menu-icon">
                    <path d="M2,3 L12,3 L12,13 L2,13 Z M5,1 L9,1 M4,6 L10,6 M4,8 L10,8 M4,10 L8,10" 
                          fill="none" stroke="currentColor" strokeWidth="1"/>
                  </svg>
                  View Changelog
                </div>
                
                <div 
                  className={`menu-item ${updateStatus === 'checking' ? 'disabled' : ''}`}
                  onClick={() => updateStatus !== 'checking' && handleMenuItemClick('check-updates')}
                >
                  <svg width="14" height="14" viewBox="0 0 14 14" className={`menu-icon ${updateStatus === 'checking' ? 'spinning' : ''}`}>
                    <path d="M7,2 L7,8 M4,5 L7,2 L10,5" stroke="currentColor" strokeWidth="1.5" 
                          strokeLinecap="round" strokeLinejoin="round" fill="none"/>
                    <path d="M2,10 L12,10 L12,12 L2,12 Z" fill="none" stroke="currentColor" strokeWidth="1"/>
                  </svg>
                  {updateStatus === 'checking' ? 'Checking for Updates..' : 'Check for Updates..'}
                </div>
              </div>
            )}
          </div>
          <button 
            className="titlebar-button minimize-button"
            onClick={handleMinimize}
            title="Minimize"
          >
            <svg width="10" height="10" viewBox="0 0 10 10">
              <path d="M0,5 L10,5" stroke="currentColor" strokeWidth="1"/>
            </svg>
          </button>
          
          <button 
            className="titlebar-button maximize-button"
            onClick={handleMaximize}
            title={isMaximized ? "Restore" : "Maximize"}
          >
            {isMaximized ? (
              <svg width="10" height="10" viewBox="0 0 10 10">
                <path d="M2,2 L8,2 L8,8 L2,8 Z M2,2 L2,0 L10,0 L10,6 L8,6" 
                      fill="none" stroke="currentColor" strokeWidth="1"/>
              </svg>
            ) : (
              <svg width="10" height="10" viewBox="0 0 10 10">
                <path d="M0,0 L10,0 L10,10 L0,10 Z" 
                      fill="none" stroke="currentColor" strokeWidth="1"/>
              </svg>
            )}
          </button>
          
          <button 
            className="titlebar-button close-button"
            onClick={handleClose}
            title="Close"
          >
            <svg width="10" height="10" viewBox="0 0 10 10">
              <path d="M0,0 L10,10 M10,0 L0,10" stroke="currentColor" strokeWidth="1"/>
            </svg>
          </button>
        </div>
      </div>
      
      {/* Update Dialog */}
      {showUpdateDialog && updateStatus === 'available' && (
        <div className="update-dialog-overlay">
          <div className="update-dialog">
            <div className="update-dialog-header">
              <h3>Update Available</h3>
              <button 
                className="update-dialog-close"
                onClick={() => setShowUpdateDialog(false)}
              >
                ×
              </button>
            </div>
            <div className="update-dialog-content">
              <p>{updateMessage}</p>
              <div className="update-dialog-actions">
                <button 
                  className="update-button secondary"
                  onClick={() => setShowUpdateDialog(false)}
                >
                  Later
                </button>
                <button 
                  className="update-button primary"
                  onClick={installUpdate}
                  disabled={updateStatus === 'downloading'}
                >
                  {updateStatus === 'downloading' ? 'Installing...' : 'Install Now'}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
      
      {/* Update Status Toast */}
      {updateMessage && updateStatus !== 'available' && (
        <div className={`update-toast ${updateStatus}`}>
          {updateMessage}
        </div>
      )}
      
      {/* Changelog Modal */}
      <ChangelogModal 
        isOpen={showChangelogModal} 
        onClose={() => setShowChangelogModal(false)} 
      />
    </div>
  );
};

export default CustomTitleBar;
