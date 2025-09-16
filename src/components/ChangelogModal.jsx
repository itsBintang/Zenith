import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './ChangelogModal.css';

const ChangelogModal = ({ isOpen, onClose }) => {
  const [changelogData, setChangelogData] = useState(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  useEffect(() => {
    if (isOpen && !changelogData) {
      fetchChangelog();
    }
  }, [isOpen, changelogData]);

  const fetchChangelog = async () => {
    setLoading(true);
    setError(null);
    
    try {
      const data = await invoke('get_changelog');
      setChangelogData(data);
    } catch (err) {
      console.error('Failed to fetch changelog:', err);
      setError('Failed to load changelog');
      // Fallback to default data
      setChangelogData({
        version: "v1.9.0",
        changes: [
          {
            type: "Added",
            description: "custom title bar with modern design",
            pr_number: "#45",
            details: "Implemented a sleek custom title bar with transparency support, window controls, and an integrated menu system for better user experience."
          },
          {
            type: "Improved",
            description: "update system with better error handling",
            pr_number: "#47", 
            details: "Enhanced the auto-update functionality with proper error handling, user-friendly messages, and progress feedback for a smoother update experience."
          }
        ]
      });
    } finally {
      setLoading(false);
    }
  };

  if (!isOpen) return null;

  const handleOverlayClick = (e) => {
    if (e.target === e.currentTarget) {
      onClose();
    }
  };

  return (
    <div className="changelog-overlay" onClick={handleOverlayClick}>
      <div className="changelog-modal">
        {/* Header */}
        <div className="changelog-header">
          <div className="changelog-title-section">
            <h2 className="changelog-title">Changes in {changelogData?.version || 'Latest Version'}</h2>
          </div>
          <button className="changelog-close" onClick={onClose}>
            ×
          </button>
        </div>

        {/* Content */}
        <div className="changelog-content">
          {loading ? (
            <div className="changelog-loading">
              <div className="loading-spinner"></div>
              <p>Loading changelog...</p>
            </div>
          ) : error ? (
            <div className="changelog-error">
              <p>⚠️ {error}</p>
              <button onClick={fetchChangelog} className="retry-button">
                Retry
              </button>
            </div>
          ) : changelogData ? (
            <div className="changelog-list">
              {Object.entries(
                changelogData.changes.reduce((acc, change) => {
                  const type = change.type || change.change_type || 'Changed';
                  if (!acc[type]) {
                    acc[type] = [];
                  }
                  acc[type].push(change);
                  return acc;
                }, {})
              ).map(([type, changes]) => (
                <div key={type} className="changelog-group">
                  <h3 className="changelog-group-title" data-type={type}>{type}</h3>
                  <ul>
                    {changes.map((change, index) => (
                      <li key={index} className="changelog-entry">
                        <span className="change-description">{change.description}</span>
                        {(change.prNumber || change.pr_number) && (
                          <a 
                            href={`https://github.com/itsBintang/Zenith/pull/${(change.pr_number || change.prNumber).replace(/[^0-9]/g, '')}`} 
                            target="_blank" 
                            rel="noopener noreferrer" 
                            className="change-pr"
                          >
                            (#{ (change.pr_number || change.prNumber).replace(/[^0-9]/g, '')})
                          </a>
                        )}
                      </li>
                    ))}
                  </ul>
                </div>
              ))}
            </div>
          ) : (
            <div className="changelog-empty">
              <p>No changelog data available</p>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="changelog-footer">
          <button className="changelog-button primary" onClick={onClose}>
            Continue
          </button>
        </div>
      </div>
    </div>
  );
};

export default ChangelogModal;
