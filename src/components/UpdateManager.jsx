import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FiDownload, FiRefreshCw, FiCheck, FiX, FiAlertCircle } from 'react-icons/fi';
import '../styles/UpdateManager.css';

function UpdateManager() {
  const [updateStatus, setUpdateStatus] = useState('idle'); // idle, checking, available, downloading, installed, error
  const [updateMessage, setUpdateMessage] = useState('');
  const [showUpdateDialog, setShowUpdateDialog] = useState(false);
  const [error, setError] = useState('');

  // Auto-check for updates on component mount
  useEffect(() => {
    console.log('🔄 UpdateManager mounted, starting auto-check...');
    checkForUpdates(false); // Silent check on startup
  }, []);

  const checkForUpdates = async (showResult = true) => {
    setUpdateStatus('checking');
    setError('');
    
    try {
      const result = await invoke('check_for_updates');
      
      if (result.includes('Update available')) {
        setUpdateStatus('available');
        setUpdateMessage(result);
        if (showResult) {
          setShowUpdateDialog(true);
        }
      } else {
        setUpdateStatus('idle');
        setUpdateMessage(result);
        if (showResult) {
          // Show "no updates" message briefly
          setTimeout(() => setUpdateMessage(''), 3000);
        }
      }
    } catch (err) {
      setUpdateStatus('error');
      setError(err.toString());
      console.error('Update check failed:', err);
    }
  };

  const installUpdate = async () => {
    setUpdateStatus('downloading');
    setError('');
    
    try {
      const result = await invoke('install_update');
      setUpdateStatus('installed');
      setUpdateMessage(result);
      
      // Show restart prompt
      setTimeout(() => {
        if (window.confirm('Update installed successfully! Would you like to restart the application now?')) {
          // In a real app, you might want to trigger a restart
          window.location.reload();
        }
      }, 1000);
      
    } catch (err) {
      setUpdateStatus('error');
      setError(err.toString());
      console.error('Update installation failed:', err);
    }
  };

  const getStatusIcon = () => {
    switch (updateStatus) {
      case 'checking':
        return <FiRefreshCw className="spinning" />;
      case 'available':
        return <FiDownload />;
      case 'downloading':
        return <FiRefreshCw className="spinning" />;
      case 'installed':
        return <FiCheck />;
      case 'error':
        return <FiAlertCircle />;
      default:
        return <FiRefreshCw />;
    }
  };

  const getStatusText = () => {
    switch (updateStatus) {
      case 'checking':
        return 'Checking for updates...';
      case 'available':
        return 'Update available';
      case 'downloading':
        return 'Downloading update...';
      case 'installed':
        return 'Update installed';
      case 'error':
        return 'Update failed';
      default:
        return 'Check for updates';
    }
  };

  return (
    <div className="update-manager">
      {/* Update Button in Header/Toolbar */}
      <button
        className={`update-button ${updateStatus}`}
        onClick={() => checkForUpdates(true)}
        disabled={updateStatus === 'checking' || updateStatus === 'downloading'}
        title={getStatusText()}
      >
        {getStatusIcon()}
        <span className="update-text">{getStatusText()}</span>
      </button>

      {/* Update Status Message */}
      {updateMessage && updateStatus !== 'available' && (
        <div className={`update-message ${updateStatus}`}>
          {updateMessage}
        </div>
      )}

      {/* Error Message */}
      {error && (
        <div className="update-error">
          <FiX />
          {error}
        </div>
      )}

      {/* Update Dialog */}
      {showUpdateDialog && updateStatus === 'available' && (
        <div className="update-dialog-overlay">
          <div className="update-dialog">
            <div className="update-dialog-header">
              <FiDownload />
              <h3>Update Available</h3>
            </div>
            
            <div className="update-dialog-body">
              <p>{updateMessage}</p>
              <p>A new version of Zenith Launcher is available. Would you like to download and install it now?</p>
            </div>
            
            <div className="update-dialog-actions">
              <button
                className="btn-secondary"
                onClick={() => setShowUpdateDialog(false)}
              >
                Later
              </button>
              <button
                className="btn-primary"
                onClick={installUpdate}
                disabled={updateStatus === 'downloading'}
              >
                {updateStatus === 'downloading' ? (
                  <>
                    <FiRefreshCw className="spinning" />
                    Installing...
                  </>
                ) : (
                  <>
                    <FiDownload />
                    Install Update
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default UpdateManager;
