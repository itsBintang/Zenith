import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FiX, FiLoader, FiChevronLeft, FiChevronRight, FiCheck, FiPlus, FiMinus, FiLock } from 'react-icons/fi';
import '../styles/DlcManager.css';

function DlcManager({ game, onClose, showNotification }) {
  const [dlcs, setDlcs] = useState([]); // Details for current page
  const [allDlcAppIds, setAllDlcAppIds] = useState([]); // All available DLC AppIDs
  const [installedDlcs, setInstalledDlcs] = useState(new Set());
  const [selectedDlcs, setSelectedDlcs] = useState(new Set());
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState(null);
  const [currentPage, setCurrentPage] = useState(1);
  const dlcsPerPage = 8;

  // Fetch initial DLC data
  useEffect(() => {
    const fetchInitialDlcData = async () => {
      setIsLoading(true);
      setError(null);
      try {
        // Get all DLC AppIDs using dedicated command
        const allIds = await invoke('get_game_dlc_list', { appId: game.app_id });

        if (allIds.length === 0) {
          setError('This game has no available DLCs.');
          setIsLoading(false);
          return;
        }
        setAllDlcAppIds(allIds);

        // Get currently installed DLCs
        const installed = await invoke('get_dlcs_in_lua', { appId: game.app_id });
        const installedSet = new Set(installed);
        setInstalledDlcs(installedSet);
        setSelectedDlcs(new Set(installedSet)); // Clone for editing
        
      } catch (err) {
        setError(`Failed to load DLC list: ${err.toString()}`);
        setAllDlcAppIds([]);
      }
    };

    fetchInitialDlcData();
  }, [game.app_id]);

  // Fetch DLC details for current page
  useEffect(() => {
    if (allDlcAppIds.length === 0) return;

    const fetchDlcPageDetails = async () => {
      setIsLoading(true);
      setDlcs([]);

      const startIndex = (currentPage - 1) * dlcsPerPage;
      const endIndex = startIndex + dlcsPerPage;
      const pageAppIds = allDlcAppIds.slice(startIndex, endIndex);

      if (pageAppIds.length === 0) {
        setIsLoading(false);
        return;
      }

      try {
        const dlcDetails = await invoke('get_batch_game_details', { appIds: pageAppIds });
        setDlcs(dlcDetails);
      } catch (err) {
        setError(`Failed to load DLC details: ${err.toString()}`);
      } finally {
        setIsLoading(false);
      }
    };

    fetchDlcPageDetails();
  }, [currentPage, allDlcAppIds]);

  const handleToggleDlc = (dlcId) => {
    setSelectedDlcs(prev => {
      const newSet = new Set(prev);
      if (newSet.has(dlcId.toString())) {
        newSet.delete(dlcId.toString());
      } else {
        newSet.add(dlcId.toString());
      }
      return newSet;
    });
  };

  const handleSaveChanges = async () => {
    setIsSaving(true);
    
    try {
      // Calculate what's being added/removed
      const previouslyInstalled = Array.from(installedDlcs);
      const newSelection = Array.from(selectedDlcs);
      
      const addedDlcs = newSelection.filter(id => !installedDlcs.has(id));
      const removedDlcs = previouslyInstalled.filter(id => !selectedDlcs.has(id));
      
      const result = await invoke('sync_dlcs_in_lua', {
        mainAppId: game.app_id,
        dlcIdsToSet: newSelection,
        addedCount: addedDlcs.length,
        removedCount: removedDlcs.length,
      });
      
      showNotification(result, 'success');
      setInstalledDlcs(new Set(selectedDlcs)); // Update installed state
      onClose();
    } catch (err) {
      showNotification(`Error saving DLCs: ${err.toString()}`, 'error');
    } finally {
      setIsSaving(false);
    }
  };

  const handleRemoveAll = async () => {
    setIsSaving(true);
    
    try {
      const removedCount = installedDlcs.size;
      
      const result = await invoke('sync_dlcs_in_lua', {
        mainAppId: game.app_id,
        dlcIdsToSet: [],
        addedCount: 0,
        removedCount: removedCount,
      });
      
      showNotification(result, 'success');
      setInstalledDlcs(new Set()); // Clear installed state
      setSelectedDlcs(new Set()); // Clear selection
      onClose();
    } catch (err) {
      showNotification(`Error removing DLCs: ${err.toString()}`, 'error');
    } finally {
      setIsSaving(false);
    }
  };

  const totalPages = Math.ceil(allDlcAppIds.length / dlcsPerPage);
  const hasChanges = !areSetsEqual(installedDlcs, selectedDlcs);

  const handlePageChange = (newPage) => {
    if (newPage >= 1 && newPage <= totalPages) {
      setCurrentPage(newPage);
    }
  };

  return (
    <div className="dlc-manager-overlay">
      <div className="dlc-manager-modal">
        {/* Header */}
        <div className="dlc-manager-header">
          <h2>DLC Unlocker</h2>
          <button onClick={onClose} className="dlc-close-btn">
            <FiX size={24} />
          </button>
        </div>

        {/* Content */}
        <div className="dlc-manager-content">
          {isLoading && (
            <div className="dlc-loading">
              <FiLoader className="spinning" size={48} />
              <p>Loading DLC Information...</p>
            </div>
          )}

          {error && !isLoading && (
            <div className="dlc-error">
              <p>{error}</p>
            </div>
          )}

          {!isLoading && !error && allDlcAppIds.length > 0 && (
            <>
              {/* Action Buttons */}
              <div className="dlc-actions-bar">
                <button 
                  className="dlc-action-btn"
                  onClick={() => {
                    const allIds = new Set(allDlcAppIds);
                    setSelectedDlcs(allIds);
                  }}
                >
                  Select All
                </button>
                <button 
                  className="dlc-action-btn"
                  onClick={() => setSelectedDlcs(new Set())}
                >
                  Select None
                </button>
                <button 
                  className="dlc-action-btn dlc-action-btn--primary"
                  onClick={handleSaveChanges}
                  disabled={isSaving || !hasChanges}
                >
                  {isSaving ? 'Saving...' : 'Unlock Selected'}
                </button>
                <button 
                  className="dlc-action-btn dlc-action-btn--danger"
                  onClick={handleRemoveAll}
                  disabled={isSaving || selectedDlcs.size === 0}
                >
                  Remove Selected
                </button>
              </div>

              {/* DLC Grid */}
              <div className="dlc-grid">
                {dlcs.map(dlc => {
                  const dlcIdStr = dlc.app_id;
                  const isSelected = selectedDlcs.has(dlcIdStr);
                  const wasInstalled = installedDlcs.has(dlcIdStr);
                  const isLocked = !wasInstalled && !isSelected;

                  return (
                    <div 
                      key={dlc.app_id} 
                      className={`dlc-card ${isSelected ? 'selected' : ''} ${isLocked ? 'locked' : ''}`}
                      onClick={() => handleToggleDlc(dlc.app_id)}
                    >
                      {/* Selection Indicator */}
                      <div className="dlc-selection-indicator">
                        {isSelected ? <FiCheck size={16} /> : <FiPlus size={16} />}
                      </div>
                      
                      {isLocked && (
                        <div className="dlc-lock-badge">
                          <FiLock size={14} />
                        </div>
                      )}

                      {/* DLC Image */}
                      <div className="dlc-card-image">
                        <img 
                          src={dlc.header_image} 
                          alt={dlc.name}
                          onError={(e) => {
                            e.target.src = `https://cdn.cloudflare.steamstatic.com/steam/apps/${dlc.app_id}/header.jpg`;
                          }}
                        />
                        {/* Title overlay */}
                        <div className="dlc-card-gradient"></div>
                        <div className="dlc-card-title-overlay">{dlc.name}</div>
                        {isLocked && (
                          <div className="dlc-card-subtitle-overlay">ID: {dlc.app_id}</div>
                        )}
                      </div>

                      {/* DLC Info removed per design: title overlays on image */}

                      {/* Status Badge */}
                      {wasInstalled && !isSelected && (
                        <div className="dlc-status-badge removing">Removing</div>
                      )}
                      {!wasInstalled && isSelected && (
                        <div className="dlc-status-badge adding">Adding</div>
                      )}
                    </div>
                  );
                })}
              </div>
            </>
          )}
        </div>

        {/* Pagination */}
        {totalPages > 1 && !error && (
          <div className="dlc-pagination">
            <button
              onClick={() => handlePageChange(currentPage - 1)}
              disabled={currentPage === 1 || isLoading}
              className="pagination-btn"
            >
              <FiChevronLeft size={16} />
              Previous
            </button>
            <span className="pagination-info">
              Page {currentPage} of {totalPages}
            </span>
            <button
              onClick={() => handlePageChange(currentPage + 1)}
              disabled={currentPage === totalPages || isLoading}
              className="pagination-btn"
            >
              Next
              <FiChevronRight size={16} />
            </button>
          </div>
        )}

      </div>
    </div>
  );
}

// Helper function to compare sets
function areSetsEqual(set1, set2) {
  if (set1.size !== set2.size) return false;
  for (let item of set1) {
    if (!set2.has(item)) return false;
  }
  return true;
}

export default DlcManager;
