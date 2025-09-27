# Download System Implementation

## Overview
Implementasi sistem download yang mirip dengan Hydra, menggunakan Rust sebagai pengganti Python RPC. Sistem ini menggunakan aria2c untuk HTTP/HTTPS downloads dan libtorrent mock untuk torrent downloads.

## Architecture

### Backend (Rust/Tauri)

#### 1. **Aria2 Service** (`src-tauri/src/download/aria2_service.rs`)
- **Purpose**: Wrapper untuk aria2c binary yang menjalankan aria2c sebagai daemon dan berkomunikasi via JSON-RPC
- **Key Features**:
  - Spawn aria2c process dengan konfigurasi optimal
  - RPC communication untuk mengelola downloads
  - Support untuk multi-connection downloads
  - Resume capability
  - Progress monitoring
  - Header dan authentication support

#### 2. **Torrent Downloader** (`src-tauri/src/download/torrent_downloader.rs`)
- **Purpose**: Mock implementation untuk torrent downloads (akan diganti dengan libtorrent bindings)
- **Key Features**:
  - Magnet link parsing
  - Info hash extraction
  - Progress simulation
  - Peer/seed tracking
  - Upload/download speed monitoring

#### 3. **Download Manager** (`src-tauri/src/download/download_manager.rs`)
- **Purpose**: Central coordinator yang memutuskan menggunakan aria2 atau torrent downloader
- **Key Features**:
  - Automatic URL type detection
  - Unified interface untuk HTTP dan torrent downloads
  - Progress monitoring dengan real-time events
  - Download state management
  - Parallel downloads support

#### 4. **Types** (`src-tauri/src/download/types.rs`)
- **Purpose**: Type definitions untuk download system
- **Key Types**:
  - `DownloadType`: Http atau Torrent
  - `DownloadStatus`: Pending, Active, Paused, Completed, Error, Cancelled, Seeding
  - `DownloadProgress`: Real-time progress information
  - `DownloadRequest`: Download request dengan URL, path, headers
  - `Aria2Config` & `TorrentConfig`: Configuration structures

#### 5. **Commands** (`src-tauri/src/download/commands.rs`)
- **Purpose**: Tauri commands untuk frontend communication
- **Available Commands**:
  - `initialize_download_manager`: Setup aria2c dan torrent session
  - `start_download`: Mulai download baru
  - `pause_download`, `resume_download`, `cancel_download`: Control downloads
  - `get_download_progress`: Get status download tertentu
  - `get_all_downloads`, `get_active_downloads`: List downloads
  - `detect_url_type`: Auto-detect apakah URL adalah HTTP atau torrent

### Frontend (React)

#### **Download Manager Component** (`src/components/DownloadManager.jsx`)
- **Purpose**: UI untuk testing dan managing downloads
- **Features**:
  - Initialize/shutdown download manager
  - Add new downloads (HTTP/torrent)
  - Real-time progress display
  - Pause/resume/cancel controls
  - Auto-refresh active downloads
  - Event listening untuk download progress dan completion

## Configuration

### Aria2c Configuration
```rust
Aria2Config {
    host: "localhost",
    port: 6800,
    secret: None,
    max_concurrent_downloads: 5,
    max_connections_per_server: 4,
    split: 4,
    min_split_size: "1M",
}
```

### Aria2c Spawn Arguments
```rust
[
    "--enable-rpc",
    "--rpc-listen-all=false",
    "--rpc-listen-port", "6800",
    "--file-allocation=none",
    "--allow-overwrite=true",
    "--auto-file-renaming=false",
    "--continue=true",
    "--max-concurrent-downloads", "5",
    "--max-connection-per-server", "4",
    "--split", "4",
    "--min-split-size", "1M",
    "--disable-ipv6=true",
    "--summary-interval=1",
]
```

## Usage

### 1. Initialize Download Manager
```javascript
await invoke('initialize_download_manager');
```

### 2. Start Download
```javascript
const downloadId = await invoke('start_download', {
    url: 'https://example.com/file.zip',
    savePath: 'C:\\Downloads',
    filename: 'myfile.zip',
    headers: { 'Authorization': 'Bearer token' },
    autoExtract: false
});
```

### 3. Monitor Progress
```javascript
// Listen for events
listen('download-progress', (event) => {
    console.log('Progress:', event.payload);
});

// Or poll manually
const progress = await invoke('get_download_progress', { downloadId });
```

### 4. Control Downloads
```javascript
await invoke('pause_download', { downloadId });
await invoke('resume_download', { downloadId });
await invoke('cancel_download', { downloadId });
```

## Dependencies

### Rust Dependencies
```toml
parking_lot = "0.12"
async-trait = "0.1"
sha1 = "0.10"
hex = "0.4"
url = "2.5"
```

### Binary Requirements
- **aria2c.exe**: Automatically bundled dalam aplikasi via Tauri resources
- **Auto-detection paths**: 
  1. Bundled resource directory (production)
  2. `C:\aria2\aria2c.exe` (fallback)
  3. Current directory `aria2c.exe`
  4. `binaries\aria2c.exe` (development)
  5. System PATH
- **Future**: libtorrent bindings untuk torrent support

## Comparison with Hydra

| Feature | Hydra (Python) | Zenith (Rust) |
|---------|----------------|---------------|
| HTTP Downloads | ✅ aria2c via Python RPC | ✅ aria2c via Rust |
| Torrent Downloads | ✅ libtorrent via Python | 🔄 Mock (will be libtorrent-rs) |
| RPC Server | ✅ Flask HTTP server | ❌ Direct IPC via Tauri |
| Progress Events | ✅ Polling + WebSocket | ✅ Tauri events |
| Multi-threading | ✅ Python threads | ✅ Tokio async |
| Memory Usage | Higher (Python) | Lower (Rust) |
| Startup Time | Slower | Faster |
| Binary Size | Larger | Smaller |

## Next Steps

1. **Real Libtorrent Integration**: Replace mock dengan proper libtorrent-rs bindings
2. **File Extraction**: Implement auto-extraction untuk ZIP/RAR files
3. **Bandwidth Management**: Add upload/download speed limits
4. **Download Queue**: Implement proper download queue management
5. **Persistence**: Save download state ke database
6. **Error Handling**: Improve error reporting dan retry mechanisms
7. **UI Improvements**: Better progress visualization dan controls

## Testing

1. Navigate ke `/downloads` di aplikasi
2. Click "Initialize Download Manager"
3. Enter URL (HTTP/HTTPS atau magnet link)
4. Set save path dan filename
5. Start download dan monitor progress
6. Test pause/resume/cancel functionality

## Build Process

### Bundling aria2c.exe

1. **Download aria2c**: Otomatis diunduh dan disimpan di `src-tauri/binaries/aria2c.exe`
2. **Tauri Configuration**: Added ke `tauri.conf.json` resources:
   ```json
   "resources": [
       "binaries/aria2c.exe"
   ]
   ```
3. **Auto-detection**: Rust code otomatis mencari aria2c.exe di berbagai lokasi
4. **Production Ready**: Binary dibundle dalam distribusi final

## Troubleshooting

### Common Issues

1. **aria2c not found**: Binary sudah dibundle otomatis, check auto-detection function
2. **Port already in use**: Change port di `Aria2Config`
3. **Downloads not starting**: Check aria2c process in Task Manager
4. **Events not received**: Verify Tauri event listeners setup correctly
5. **Import errors**: Use `@tauri-apps/api/core` instead of `@tauri-apps/api/tauri`

### Debug Commands

```bash
# Check if aria2c is running
tasklist | findstr aria2c

# Check port usage
netstat -an | findstr 6800
```
