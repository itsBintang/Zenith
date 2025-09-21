import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FiX, FiLoader, FiChevronLeft, FiChevronRight, FiCheck, FiPlus, FiMinus, FiLock, FiRefreshCw } from 'react-icons/fi';
import '../styles/DlcManager.css';

function DlcManager({ game, onClose, showNotification }) {
  const [dlcs, setDlcs] = useState([]); // Details for current page
  const [allDlcAppIds, setAllDlcAppIds] = useState([]); // All available DLC AppIDs
  const [installedDlcs, setInstalledDlcs] = useState(new Set());
  const [selectedDlcs, setSelectedDlcs] = useState(new Set());
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
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

        // Validate and sanitize DLC IDs
        const validIds = Array.isArray(allIds) ? allIds.filter(id => id && (typeof id === 'string' || typeof id === 'number')).map(id => String(id)) : [];
        
        if (validIds.length === 0) {
          setError('This game has no available DLCs.');
          setIsLoading(false);
          return;
        }
        setAllDlcAppIds(validIds);

        // Get currently installed DLCs
        const installed = await invoke('get_dlcs_in_lua', { appId: game.app_id });
        const validInstalled = Array.isArray(installed) ? installed.filter(id => id && (typeof id === 'string' || typeof id === 'number')).map(id => String(id)) : [];
        const installedSet = new Set(validInstalled);
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
        
        // Validate and sanitize DLC details
        const sanitizedDetails = dlcDetails.filter(dlc => dlc && dlc.app_id).map(dlc => ({
          app_id: String(dlc.app_id || ''),
          name: String(dlc.name || 'Unknown DLC'),
          header_image: String(dlc.header_image || ''),
          // Add other fields as needed, ensuring they're all strings
        }));
        
        setDlcs(sanitizedDetails);
      } catch (err) {
        console.error('Error fetching DLC details:', err);
        setError(`Failed to load DLC details: ${err.toString()}`);
        // Set empty array as fallback
        setDlcs([]);
      } finally {
        setIsLoading(false);
      }
    };

    fetchDlcPageDetails();
  }, [currentPage, allDlcAppIds]);

  // Refresh DLC cache and reload data
  const refreshDlcCache = async () => {
    try {
      setIsRefreshing(true);
      setError(null);
      
      // Force refresh DLC cache from Steam API
      await invoke('refresh_dlc_cache', { appId: game.app_id });
      console.log('DLC cache refreshed from Steam API');
      
      // Reload DLC data
      const allIds = await invoke('get_game_dlc_list', { appId: game.app_id });
      
      // Validate DLC IDs are strings
      const validIds = Array.isArray(allIds) ? allIds.filter(id => id && typeof id === 'string' || typeof id === 'number').map(id => String(id)) : [];
      setAllDlcAppIds(validIds);
      
      // Reset to first page
      setCurrentPage(1);
      
      // Get currently installed DLCs
      const installed = await invoke('get_dlcs_in_lua', { appId: game.app_id });
      const validInstalled = Array.isArray(installed) ? installed.filter(id => id && typeof id === 'string' || typeof id === 'number').map(id => String(id)) : [];
      const installedSet = new Set(validInstalled);
      setInstalledDlcs(installedSet);
      setSelectedDlcs(new Set(installedSet));
      
      showNotification?.(`Successfully refreshed ${allIds.length} DLCs for ${game.name}`, 'success');
    } catch (error) {
      console.error('Error refreshing DLC cache:', error);
      const errorMessage = typeof error === 'string' ? error : error.toString();
      setError(`Failed to refresh DLC cache: ${errorMessage}`);
      showNotification?.(errorMessage, 'error');
    } finally {
      setIsRefreshing(false);
    }
  };

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
          <div className="dlc-manager-header-left">
            <h2>DLC Unlocker</h2>
          </div>
          <div className="dlc-manager-header-right">
            <button 
              className={`dlc-refresh-btn ${isRefreshing ? 'loading' : ''}`}
              onClick={refreshDlcCache}
              disabled={isRefreshing || isLoading || isSaving}
              title="Refresh DLC cache from Steam API"
            >
              <FiRefreshCw size={16} />
            </button>
            <button onClick={onClose} className="dlc-close-btn">
              <FiX size={24} />
            </button>
          </div>
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
                {Array.isArray(dlcs) && dlcs.map(dlc => {
                  // Ensure all data is properly formatted and not objects
                  const dlcIdStr = String(dlc.app_id || '');
                  const dlcName = String(dlc.name || 'Unknown DLC');
                  const dlcImage = String(dlc.header_image || '');
                  
                  const isSelected = selectedDlcs.has(dlcIdStr);
                  const wasInstalled = installedDlcs.has(dlcIdStr);
                  const isLocked = !wasInstalled && !isSelected;

                  return (
                    <div 
                      key={dlcIdStr} 
                      className={`dlc-card ${isSelected ? 'selected' : ''} ${isLocked ? 'locked' : ''}`}
                      onClick={() => handleToggleDlc(dlcIdStr)}
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
                          src={dlcImage} 
                          alt={dlcName}
                          onError={(e) => {
                            e.target.src = `https://cdn.cloudflare.steamstatic.com/steam/apps/${dlcIdStr}/header.jpg`;
                          }}
                        />
                        {/* Title overlay */}
                        <div className="dlc-card-gradient"></div>
                        <div className="dlc-card-title-overlay">{dlcName}</div>
                        {isLocked && (
                          <div className="dlc-card-subtitle-overlay">ID: {dlcIdStr}</div>
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
