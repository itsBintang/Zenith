import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import DownloadHistory from './DownloadHistory';

const DownloadManager = () => {
  const [activeTab, setActiveTab] = useState('active'); // 'active' or 'history'
  const [isInitialized, setIsInitialized] = useState(false);
  const [downloads, setDownloads] = useState([]);
  const [url, setUrl] = useState('');
  const [savePath, setSavePath] = useState('');
  const [fileName, setFileName] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    // Check if download manager is already ready before initializing
    checkDownloadManagerStatus();
    
    // Listen for download progress events
    const unlisten = listen('download-progress', (event) => {
      // Only log significant progress changes (every 10%)
      const progress = event.payload;
      if (progress.progress % 0.1 < 0.05 || progress.status !== 'Active') {
        console.log('Download progress:', progress.download_id, `${(progress.progress * 100).toFixed(1)}%`);
      }
      updateDownloadProgress(event.payload);
    });

    const unlistenComplete = listen('download-complete', (event) => {
      console.log('Download complete:', event.payload.download_id);
      setError('Download completed: ' + event.payload.file_name);
    });

    // Auto-refresh downloads every 5 seconds (less aggressive)
    const refreshInterval = setInterval(() => {
      if (isInitialized) {
        loadDownloads();
      }
    }, 5000);

    return () => {
      unlisten.then(f => f());
      unlistenComplete.then(f => f());
      clearInterval(refreshInterval);
      // Don't auto-shutdown on unmount to prevent conflicts
    };
  }, []);

  const checkDownloadManagerStatus = async () => {
    try {
      // Check if download manager is already ready
      const isReady = await invoke('is_download_manager_ready');
      if (isReady) {
        console.log('Download manager already initialized');
        setIsInitialized(true);
        setError('Download manager already ready');
        await loadDownloads();
      } else {
        console.log('Download manager not ready, need to initialize');
        setIsInitialized(false);
      }
    } catch (err) {
      console.error('Failed to check download manager status:', err);
      setIsInitialized(false);
    }
  };

  const initializeDownloadManager = async () => {
    try {
      setIsLoading(true);
      
      // Double check status first
      const isReady = await invoke('is_download_manager_ready');
      if (isReady) {
        console.log('Download manager already ready, skipping initialization');
        setIsInitialized(true);
        setError('Download manager was already ready');
        await loadDownloads();
        return;
      }
      
      const result = await invoke('initialize_download_manager');
      console.log('Download manager initialized:', result);
      setIsInitialized(true);
      setError('Download manager initialized successfully');
      
      // Load existing downloads
      await loadDownloads();
    } catch (err) {
      console.error('Failed to initialize download manager:', err);
      setError('Failed to initialize: ' + err);
      setIsInitialized(false);
    } finally {
      setIsLoading(false);
    }
  };

  const shutdownDownloadManager = async () => {
    try {
      await invoke('shutdown_download_manager');
      console.log('Download manager shut down');
      setIsInitialized(false);
      setDownloads([]); // Clear all downloads from UI
      setError('Download manager shut down - all downloads cleared');
    } catch (err) {
      console.error('Failed to shutdown download manager:', err);
    }
  };

   const loadDownloads = async () => {
     try {
       const activeDownloads = await invoke('get_active_downloads');
       
       // Filter out completed downloads for cleaner UI
       const ongoingDownloads = activeDownloads.filter(download => 
         download.status !== 'Completed' && download.progress < 1.0
       );
       
       // Only log when there are changes
       if (ongoingDownloads.length !== downloads.length) {
         console.log('Downloads updated:', ongoingDownloads.length, 'active downloads');
       }
       
       setDownloads(ongoingDownloads);
     } catch (err) {
       console.error('Failed to load downloads:', err);
     }
   };

  const updateDownloadProgress = (progress) => {
    setDownloads(prev => {
      const index = prev.findIndex(d => d.download_id === progress.download_id);
      
      // Auto-remove completed downloads after 3 seconds
      if (progress.status === 'Completed' || progress.progress >= 1.0) {
        setTimeout(() => {
          setDownloads(current => current.filter(d => d.download_id !== progress.download_id));
        }, 3000);
      }
      
      if (index >= 0) {
        const newDownloads = [...prev];
        newDownloads[index] = progress;
        return newDownloads;
      } else {
        // Only add if not completed
        if (progress.status !== 'Completed' && progress.progress < 1.0) {
          return [...prev, progress];
        }
        return prev;
      }
    });
  };

  const startDownload = async () => {
    if (!url.trim()) {
      setError('Please enter a URL');
      return;
    }

    try {
      setIsLoading(true);
      setError('');
      
      const downloadId = await invoke('start_download', {
        url: url.trim(),
        savePath: savePath.trim() || 'C:\\Downloads',
        filename: fileName.trim() || null,
        headers: null,
        autoExtract: false
      });
      
      console.log('Download started with ID:', downloadId);
      setError('Download started successfully');
      
      // Clear form
      setUrl('');
      setFileName('');
      
      // Refresh downloads list
      await loadDownloads();
    } catch (err) {
      console.error('Failed to start download:', err);
      setError('Failed to start download: ' + err);
    } finally {
      setIsLoading(false);
    }
  };

  const pauseDownload = async (downloadId) => {
    try {
      console.log('Attempting to pause download:', downloadId);
      await invoke('pause_download', { downloadId });
      setError('Download paused successfully');
      
      // Wait a bit before refreshing to let the backend update
      setTimeout(() => {
        loadDownloads();
      }, 1000);
    } catch (err) {
      console.error('Failed to pause download:', err);
      setError('Failed to pause download: ' + err);
    }
  };

  const resumeDownload = async (downloadId) => {
    try {
      console.log('Attempting to resume download:', downloadId);
      await invoke('resume_download', { downloadId });
      setError('Download resumed successfully');
      
      // Wait a bit before refreshing to let the backend update
      setTimeout(() => {
        loadDownloads();
      }, 1000);
    } catch (err) {
      console.error('Failed to resume download:', err);
      setError('Failed to resume download: ' + err);
    }
  };

  const cancelDownload = async (downloadId) => {
    try {
      await invoke('cancel_download', { downloadId });
      setError('Download cancelled');
      await loadDownloads();
    } catch (err) {
      console.error('Failed to cancel download:', err);
      setError('Failed to cancel: ' + err);
    }
  };

  const detectUrlType = async () => {
    if (!url.trim()) return;
    
    try {
      const type = await invoke('detect_url_type', { url: url.trim() });
      setError(`URL type detected: ${type}`);
    } catch (err) {
      console.error('Failed to detect URL type:', err);
    }
  };

  const clearCompletedDownloads = () => {
    setDownloads(prev => prev.filter(download => 
      download.status !== 'Completed' && download.progress < 1.0
    ));
    setError('Completed downloads cleared');
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
    return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  };

  return (
    <div style={{ padding: '20px', maxWidth: '1200px', margin: '0 auto' }}>
      <div style={{ marginBottom: '20px' }}>
        <h1>Download Manager</h1>
        
        {/* Tab Navigation */}
        <div style={{ display: 'flex', borderBottom: '2px solid #e9ecef', marginBottom: '20px' }}>
          <button
            onClick={() => setActiveTab('active')}
            style={{
              padding: '10px 20px',
              border: 'none',
              backgroundColor: activeTab === 'active' ? '#007bff' : 'transparent',
              color: activeTab === 'active' ? 'white' : '#007bff',
              borderRadius: '4px 4px 0 0',
              cursor: 'pointer',
              marginRight: '5px',
              fontWeight: activeTab === 'active' ? 'bold' : 'normal'
            }}
          >
            Active Downloads
          </button>
          <button
            onClick={() => setActiveTab('history')}
            style={{
              padding: '10px 20px',
              border: 'none',
              backgroundColor: activeTab === 'history' ? '#007bff' : 'transparent',
              color: activeTab === 'history' ? 'white' : '#007bff',
              borderRadius: '4px 4px 0 0',
              cursor: 'pointer',
              fontWeight: activeTab === 'history' ? 'bold' : 'normal'
            }}
          >
            Download History
          </button>
        </div>
      </div>

      {/* Tab Content */}
      {activeTab === 'active' ? (
        <div>
      
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

      <div style={{ marginBottom: '20px' }}>
        <h3>Status: {isInitialized ? '✅ Ready' : '❌ Not Ready'}</h3>
        
        {!isInitialized && (
          <button 
            onClick={initializeDownloadManager}
            disabled={isLoading}
            style={{ padding: '10px 20px', marginRight: '10px' }}
          >
            {isLoading ? 'Initializing...' : 'Initialize Download Manager'}
          </button>
        )}
        
        {isInitialized && (
          <>
            <button 
              onClick={clearCompletedDownloads}
              style={{ padding: '10px 20px', backgroundColor: '#6c757d', color: 'white', marginRight: '10px' }}
            >
              Clear Completed
            </button>
            <button 
              onClick={shutdownDownloadManager}
              style={{ padding: '10px 20px', backgroundColor: '#dc3545', color: 'white' }}
            >
              Shutdown Download Manager
            </button>
          </>
        )}
      </div>

      {isInitialized && (
        <div style={{ marginBottom: '30px' }}>
          <h3>Add New Download</h3>
          <div style={{ marginBottom: '10px' }}>
            <input
              type="text"
              placeholder="Enter URL (HTTP/HTTPS or magnet:)"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              style={{ width: '100%', padding: '8px', marginBottom: '10px' }}
            />
            <input
              type="text"
              placeholder="Save path (optional, default: C:\\Downloads)"
              value={savePath}
              onChange={(e) => setSavePath(e.target.value)}
              style={{ width: '100%', padding: '8px', marginBottom: '10px' }}
            />
            <input
              type="text"
              placeholder="File name (optional)"
              value={fileName}
              onChange={(e) => setFileName(e.target.value)}
              style={{ width: '100%', padding: '8px', marginBottom: '10px' }}
            />
            <div>
              <button 
                onClick={startDownload}
                disabled={isLoading || !url.trim()}
                style={{ padding: '10px 20px', marginRight: '10px', backgroundColor: '#28a745', color: 'white' }}
              >
                {isLoading ? 'Starting...' : 'Start Download'}
              </button>
              <button 
                onClick={detectUrlType}
                disabled={!url.trim()}
                style={{ padding: '10px 20px' }}
              >
                Detect URL Type
              </button>
            </div>
          </div>
        </div>
      )}

      {isInitialized && (
        <div>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px' }}>
            <h3>Active Downloads ({downloads.length})</h3>
            <button onClick={loadDownloads} style={{ padding: '5px 10px' }}>
              Refresh
            </button>
          </div>
          
          {downloads.length === 0 ? (
            <p>No active downloads</p>
          ) : (
            <div>
              {downloads.map((download) => (
                <div 
                  key={download.download_id} 
                  style={{ 
                    border: '1px solid #ddd', 
                    padding: '15px', 
                    marginBottom: '10px', 
                    borderRadius: '5px',
                    backgroundColor: '#f9f9f9'
                  }}
                >
                  <div style={{ marginBottom: '10px' }}>
                    <strong>ID:</strong> {download.download_id}<br/>
                    <strong>File:</strong> {download.file_name || 'Unknown'}<br/>
                    <strong>Status:</strong> {download.status}<br/>
                    <strong>Progress:</strong> {(download.progress * 100).toFixed(1)}%<br/>
                    <strong>Size:</strong> {formatBytes(download.downloaded_size)} / {formatBytes(download.total_size)}<br/>
                    <strong>Speed:</strong> ↓ {formatSpeed(download.download_speed)} ↑ {formatSpeed(download.upload_speed)}<br/>
                    <strong>ETA:</strong> {download.eta ? formatTime(download.eta) : 'Unknown'}<br/>
                    <strong>Peers:</strong> {download.num_peers} / Seeds: {download.num_seeds}
                  </div>
                  
                  <div style={{ width: '100%', backgroundColor: '#e9ecef', borderRadius: '4px', marginBottom: '10px' }}>
                    <div 
                      style={{ 
                        width: `${download.progress * 100}%`, 
                        height: '20px', 
                        backgroundColor: '#007bff', 
                        borderRadius: '4px',
                        transition: 'width 0.3s ease'
                      }}
                    />
                  </div>
                  
                  <div>
                    {download.status === 'Active' ? (
                      <button 
                        onClick={() => pauseDownload(download.download_id)}
                        style={{ padding: '5px 10px', marginRight: '5px', backgroundColor: '#ffc107', color: 'black' }}
                      >
                        Pause
                      </button>
                    ) : download.status === 'Paused' ? (
                      <button 
                        onClick={() => resumeDownload(download.download_id)}
                        style={{ padding: '5px 10px', marginRight: '5px', backgroundColor: '#28a745', color: 'white' }}
                      >
                        Resume
                      </button>
                    ) : null}
                    
                    <button 
                      onClick={() => cancelDownload(download.download_id)}
                      style={{ padding: '5px 10px', backgroundColor: '#dc3545', color: 'white' }}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
        </div>
      ) : (
        <DownloadHistory />
      )}
    </div>
  );
};

export default DownloadManager;
