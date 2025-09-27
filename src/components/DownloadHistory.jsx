import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FiTrash2, FiSearch, FiDownload, FiClock, FiCheckCircle, FiXCircle, FiPauseCircle } from 'react-icons/fi';

const DownloadHistory = () => {
  const [history, setHistory] = useState([]);
  const [stats, setStats] = useState(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState('');
  const [searchTerm, setSearchTerm] = useState('');
  const [filterType, setFilterType] = useState(''); // '', 'bypass', 'regular'
  const [currentPage, setCurrentPage] = useState(0);
  const [hasMore, setHasMore] = useState(true);
  
  const ITEMS_PER_PAGE = 20;

  useEffect(() => {
    loadHistory();
    loadStats();
  }, [filterType, currentPage]);

  useEffect(() => {
    // Reset pagination when filter changes
    setCurrentPage(0);
    setHistory([]);
    loadHistory(true);
  }, [filterType]);

  const loadHistory = async (reset = false) => {
    try {
      setIsLoading(true);
      const offset = reset ? 0 : currentPage * ITEMS_PER_PAGE;
      
      const newHistory = await invoke('get_download_history', {
        limit: ITEMS_PER_PAGE,
        offset: offset,
        filterType: filterType || null
      });

      if (reset) {
        setHistory(newHistory);
      } else {
        setHistory(prev => [...prev, ...newHistory]);
      }
      
      setHasMore(newHistory.length === ITEMS_PER_PAGE);
    } catch (err) {
      console.error('Failed to load history:', err);
      setError('Failed to load download history: ' + err);
    } finally {
      setIsLoading(false);
    }
  };

  const loadStats = async () => {
    try {
      const historyStats = await invoke('get_download_history_stats');
      setStats(historyStats);
    } catch (err) {
      console.error('Failed to load history stats:', err);
    }
  };

  const searchHistory = async () => {
    if (!searchTerm.trim()) {
      loadHistory(true);
      return;
    }

    try {
      setIsLoading(true);
      const searchResults = await invoke('search_download_history', {
        searchTerm: searchTerm.trim(),
        limit: 50
      });
      setHistory(searchResults);
      setHasMore(false);
    } catch (err) {
      console.error('Failed to search history:', err);
      setError('Failed to search history: ' + err);
    } finally {
      setIsLoading(false);
    }
  };

  const deleteHistoryEntry = async (id) => {
    if (!confirm('Are you sure you want to delete this download history entry?')) {
      return;
    }

    try {
      const deleted = await invoke('delete_download_history_entry', { id });
      if (deleted) {
        setHistory(prev => prev.filter(item => item.id !== id));
        setError('History entry deleted successfully');
        loadStats(); // Refresh stats
      }
    } catch (err) {
      console.error('Failed to delete history entry:', err);
      setError('Failed to delete history entry: ' + err);
    }
  };

  const clearAllHistory = async () => {
    if (!confirm(`Are you sure you want to clear ${filterType ? filterType + ' ' : 'all '}download history? This cannot be undone.`)) {
      return;
    }

    try {
      const deletedCount = await invoke('clear_download_history', {
        filterType: filterType || null
      });
      setHistory([]);
      setError(`Cleared ${deletedCount} history entries`);
      loadStats(); // Refresh stats
    } catch (err) {
      console.error('Failed to clear history:', err);
      setError('Failed to clear history: ' + err);
    }
  };

  const debugDatabase = async () => {
    try {
      const debugInfo = await invoke('debug_history_database');
      alert(debugInfo);
    } catch (err) {
      console.error('Failed to debug database:', err);
      alert('Failed to debug database: ' + err);
    }
  };

  const redownload = async (historyEntry) => {
    try {
      const result = await invoke('redownload_from_history', {
        historyId: historyEntry.id,
        newSavePath: null // Use original path
      });
      setError('Re-download initiated: ' + result);
    } catch (err) {
      console.error('Failed to re-download:', err);
      setError('Failed to re-download: ' + err);
    }
  };

  const loadMore = () => {
    if (!isLoading && hasMore) {
      setCurrentPage(prev => prev + 1);
    }
  };

  const formatBytes = (bytes) => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const formatSpeed = (bytesPerSecond) => {
    return formatBytes(bytesPerSecond) + '/s';
  };

  const formatTime = (seconds) => {
    if (!seconds) return 'Unknown';
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    
    if (hours > 0) {
      return `${hours}h ${minutes}m ${secs}s`;
    } else if (minutes > 0) {
      return `${minutes}m ${secs}s`;
    } else {
      return `${secs}s`;
    }
  };

  const formatDate = (timestamp) => {
    if (!timestamp) return 'Unknown';
    return new Date(timestamp * 1000).toLocaleString();
  };

  const getStatusIcon = (status) => {
    switch (status.toLowerCase()) {
      case 'completed': return <FiCheckCircle className="status-icon completed" />;
      case 'failed': return <FiXCircle className="status-icon failed" />;
      case 'cancelled': return <FiPauseCircle className="status-icon cancelled" />;
      default: return <FiClock className="status-icon pending" />;
    }
  };

  const getStatusColor = (status) => {
    switch (status.toLowerCase()) {
      case 'completed': return '#28a745';
      case 'failed': return '#dc3545';
      case 'cancelled': return '#ffc107';
      default: return '#6c757d';
    }
  };

  return (
    <div style={{ padding: '20px', maxWidth: '1200px', margin: '0 auto' }}>
      <div style={{ marginBottom: '20px' }}>
        <h2>Download History</h2>
        
        {/* Statistics */}
        {stats && (
          <div style={{ 
            display: 'grid', 
            gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', 
            gap: '15px', 
            marginBottom: '20px',
            padding: '15px',
            backgroundColor: '#f8f9fa',
            borderRadius: '8px'
          }}>
            <div>
              <strong>Total Downloads:</strong> {stats.total_downloads}
            </div>
            <div>
              <strong>Completed:</strong> {stats.completed_downloads}
            </div>
            <div>
              <strong>Failed:</strong> {stats.failed_downloads}
            </div>
            <div>
              <strong>Data Downloaded:</strong> {stats.total_data_downloaded_gb.toFixed(2)} GB
            </div>
            <div>
              <strong>Bypass Downloads:</strong> {stats.bypass_downloads}
            </div>
            <div>
              <strong>Regular Downloads:</strong> {stats.regular_downloads}
            </div>
            <div>
              <strong>Avg Speed:</strong> {stats.avg_download_speed_mbps.toFixed(2)} MB/s
            </div>
            <div>
              <strong>Total Time:</strong> {stats.total_download_time_hours.toFixed(2)} hours
            </div>
          </div>
        )}

        {/* Controls */}
        <div style={{ display: 'flex', gap: '10px', marginBottom: '20px', flexWrap: 'wrap' }}>
          <div style={{ display: 'flex', gap: '10px', flex: 1 }}>
            <input
              type="text"
              placeholder="Search by game name, file name, or App ID..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              onKeyPress={(e) => e.key === 'Enter' && searchHistory()}
              style={{ flex: 1, padding: '8px', borderRadius: '4px', border: '1px solid #ddd' }}
            />
            <button 
              onClick={searchHistory}
              style={{ padding: '8px 16px', backgroundColor: '#007bff', color: 'white', border: 'none', borderRadius: '4px' }}
            >
              <FiSearch size={16} />
            </button>
          </div>
          
          <select
            value={filterType}
            onChange={(e) => setFilterType(e.target.value)}
            style={{ padding: '8px', borderRadius: '4px', border: '1px solid #ddd' }}
          >
            <option value="">All Downloads</option>
            <option value="bypass">Bypass Downloads</option>
            <option value="regular">Regular Downloads</option>
          </select>
          
          <button 
            onClick={clearAllHistory}
            style={{ padding: '8px 16px', backgroundColor: '#dc3545', color: 'white', border: 'none', borderRadius: '4px' }}
          >
            <FiTrash2 size={16} /> Clear {filterType ? filterType.charAt(0).toUpperCase() + filterType.slice(1) : 'All'}
          </button>
          
          <button 
            onClick={debugDatabase}
            style={{ padding: '8px 16px', backgroundColor: '#6c757d', color: 'white', border: 'none', borderRadius: '4px' }}
          >
            Debug DB
          </button>
        </div>
      </div>

      {error && (
        <div style={{ 
          padding: '10px', 
          marginBottom: '10px', 
          backgroundColor: error.includes('success') ? '#d4edda' : '#f8d7da',
          color: error.includes('success') ? '#155724' : '#721c24',
          border: `1px solid ${error.includes('success') ? '#c3e6cb' : '#f5c6cb'}`,
          borderRadius: '4px'
        }}>
          {error}
        </div>
      )}

      {/* History List */}
      <div>
        {history.length === 0 ? (
          <div style={{ textAlign: 'center', padding: '40px', color: '#6c757d' }}>
            {isLoading ? 'Loading history...' : 'No download history found'}
          </div>
        ) : (
          <>
            {history.map((entry) => (
              <div 
                key={entry.id} 
                style={{ 
                  border: '1px solid #ddd', 
                  padding: '15px', 
                  marginBottom: '10px', 
                  borderRadius: '8px',
                  backgroundColor: '#ffffff'
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '10px' }}>
                  <div style={{ flex: 1 }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '5px' }}>
                      {getStatusIcon(entry.status)}
                      <strong style={{ color: getStatusColor(entry.status) }}>
                        {entry.file_name || 'Unknown File'}
                      </strong>
                      <span style={{ 
                        padding: '2px 8px', 
                        backgroundColor: entry.download_type === 'bypass' ? '#28a745' : '#007bff',
                        color: 'white',
                        borderRadius: '12px',
                        fontSize: '12px'
                      }}>
                        {entry.download_type}
                      </span>
                    </div>
                    
                    {entry.game_name && (
                      <div style={{ color: '#6c757d', marginBottom: '5px' }}>
                        <strong>Game:</strong> {entry.game_name} {entry.app_id && `(${entry.app_id})`}
                      </div>
                    )}
                    
                    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '10px', fontSize: '14px', color: '#6c757d' }}>
                      <div><strong>Size:</strong> {formatBytes(entry.file_size_mb * 1048576)}</div>
                      <div><strong>Progress:</strong> {entry.progress_percent.toFixed(1)}%</div>
                      <div><strong>Speed:</strong> {formatSpeed(entry.avg_speed_mbps * 1048576)}</div>
                      <div><strong>Time:</strong> {formatTime(entry.total_time_seconds)}</div>
                      <div><strong>Started:</strong> {formatDate(entry.started_at)}</div>
                      <div><strong>Completed:</strong> {entry.completed_at ? formatDate(entry.completed_at) : 'Not completed'}</div>
                    </div>
                  </div>
                  
                  <div style={{ display: 'flex', gap: '8px', marginLeft: '15px' }}>
                    {entry.is_redownloadable && entry.status === 'completed' && (
                      <button 
                        onClick={() => redownload(entry)}
                        style={{ 
                          padding: '6px 12px', 
                          backgroundColor: '#28a745', 
                          color: 'white', 
                          border: 'none', 
                          borderRadius: '4px',
                          fontSize: '12px'
                        }}
                        title="Re-download this file"
                      >
                        <FiDownload size={14} />
                      </button>
                    )}
                    
                    <button 
                      onClick={() => deleteHistoryEntry(entry.id)}
                      style={{ 
                        padding: '6px 12px', 
                        backgroundColor: '#dc3545', 
                        color: 'white', 
                        border: 'none', 
                        borderRadius: '4px',
                        fontSize: '12px'
                      }}
                      title="Delete this history entry"
                    >
                      <FiTrash2 size={14} />
                    </button>
                  </div>
                </div>
              </div>
            ))}
            
            {hasMore && (
              <div style={{ textAlign: 'center', marginTop: '20px' }}>
                <button 
                  onClick={loadMore}
                  disabled={isLoading}
                  style={{ 
                    padding: '10px 20px', 
                    backgroundColor: '#007bff', 
                    color: 'white', 
                    border: 'none', 
                    borderRadius: '4px'
                  }}
                >
                  {isLoading ? 'Loading...' : 'Load More'}
                </button>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
};

export default DownloadHistory;
