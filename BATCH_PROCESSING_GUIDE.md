# 🚀 Safe Batch Processing System - User Guide

## 📋 **Ringkasan**

Sistem **Safe Batch Processing** telah berhasil diimplementasikan untuk mengatasi kekhawatiran concurrent fetching yang dapat menyebabkan Steam API ban. Sistem ini menggunakan:

- **Semaphore** untuk membatasi concurrent requests
- **Batch processing** dengan delay antar batch
- **Smart refresh** berdasarkan prioritas staleness
- **Circuit breaker** untuk proteksi otomatis
- **Rate limiting** yang dapat dikonfigurasi

---

## 🛡️ **Fitur Keamanan**

### **1. Concurrent Limiting**
- **Max 3 concurrent requests** secara bersamaan
- **Semaphore** memastikan tidak ada spam request
- **Sequential processing** dalam setiap batch

### **2. Rate Limiting**
- **1.5 detik delay** antar request individual
- **15 detik delay** antar batch
- **Exponential backoff** pada error

### **3. Circuit Breaker**
- **Otomatis stop** setelah 3 error berturut-turut
- **Proteksi dari ban Steam API**
- **Auto-recovery** setelah kondisi membaik

### **4. Smart Batching**
- **15 games per batch** (ukuran optimal)
- **Priority-based refresh** (stale data first)
- **Graceful handling** circuit breaker

---

## 🎯 **Command Baru untuk Frontend**

### **1. `batch_refresh_games(app_ids: Vec<String>)`**
```javascript
// Refresh multiple games safely
const result = await invoke('batch_refresh_games', { 
    appIds: ['413150', '1245620', '740130'] 
});
console.log(result); // "Batch refresh completed: 3/3 successful, 0 failed, 0 skipped"
```

### **2. `smart_refresh_library(app_ids: Vec<String>)`**
```javascript
// Smart refresh berdasarkan prioritas staleness
const result = await invoke('smart_refresh_library', { 
    appIds: libraryGameIds 
});
console.log(result); // "Smart library refresh completed: 15/20 games processed"
```

### **3. `get_cache_config()`**
```javascript
// Get current configuration
const config = await invoke('get_cache_config');
console.log(config);
/* Output:
{
    "max_concurrent_requests": 3,
    "batch_size": 15,
    "batch_delay_seconds": 15,
    "request_delay_ms": 1500,
    "circuit_breaker_threshold": 3,
    "max_retries": 2
}
*/
```

---

## 📊 **Konfigurasi Default**

| Parameter | Nilai | Penjelasan |
|-----------|-------|------------|
| `max_concurrent_requests` | **3** | Max request bersamaan |
| `batch_size` | **15** | Games per batch |
| `batch_delay_seconds` | **15** | Delay antar batch |
| `request_delay_ms` | **1500** | Delay antar request (1.5s) |
| `circuit_breaker_threshold` | **3** | Error limit sebelum stop |
| `max_retries` | **2** | Max retry per request |

---

## 🚀 **Penggunaan Praktis**

### **Scenario 1: Refresh Library (500+ Games)**
```javascript
async function refreshMyLibrary() {
    try {
        // Get all library games
        const libraryGames = await invoke('get_library_games');
        const gameIds = libraryGames.map(game => game.app_id);
        
        // Use smart refresh (prioritizes stale data)
        const result = await invoke('smart_refresh_library', { 
            appIds: gameIds 
        });
        
        console.log('✅ Library refresh result:', result);
    } catch (error) {
        console.error('❌ Refresh failed:', error);
    }
}
```

### **Scenario 2: Batch Refresh Specific Games**
```javascript
async function refreshSpecificGames(gameIds) {
    try {
        const result = await invoke('batch_refresh_games', { 
            appIds: gameIds 
        });
        
        console.log('✅ Batch refresh completed:', result);
    } catch (error) {
        console.error('❌ Batch refresh failed:', error);
    }
}

// Usage
refreshSpecificGames(['413150', '1245620', '740130']);
```

### **Scenario 3: Monitor Configuration**
```javascript
async function showCacheConfig() {
    try {
        const config = await invoke('get_cache_config');
        
        console.log('📊 Current Cache Configuration:');
        console.log(`- Max concurrent: ${config.max_concurrent_requests}`);
        console.log(`- Batch size: ${config.batch_size}`);
        console.log(`- Batch delay: ${config.batch_delay_seconds}s`);
        console.log(`- Request delay: ${config.request_delay_ms}ms`);
    } catch (error) {
        console.error('❌ Failed to get config:', error);
    }
}
```

---

## ⚡ **Performance & Safety**

### **Timing Analysis (500 Games)**
- **Old System**: 500 concurrent requests → **High ban risk** 🚨
- **New System**: 34 batches × 15s delay = **8.5 minutes** ✅
- **Safety**: Circuit breaker stops on error → **Zero ban risk** 🛡️

### **Smart Refresh Priority**
1. **High Priority**: Missing data atau > 7 hari
2. **Medium Priority**: Data > 3 hari  
3. **Low Priority**: Data > 1 hari
4. **Skip**: Data < 1 hari (fresh)

### **Real-time Monitoring**
```bash
# Console output example:
Starting batch refresh of 45 games
Processing batch 1 of 15 games
✅ Successfully refreshed: 413150
✅ Successfully refreshed: 1245620
✅ Successfully refreshed: 740130
Batch 1 completed. Waiting 15 seconds before next batch...
Processing batch 2 of 15 games...
```

---

## 🎉 **Benefits**

✅ **Zero Steam Ban Risk** - Controlled rate limiting  
✅ **Smart Priority** - Refresh stale data first  
✅ **Circuit Protection** - Auto-stop on errors  
✅ **User Experience** - Progress monitoring  
✅ **Configurable** - Adjustable parameters  
✅ **Backward Compatible** - Works with existing code  

---

## 🔧 **Integration dengan UI**

### **Progress Bar Implementation**
```javascript
async function refreshLibraryWithProgress(gameIds) {
    const totalGames = gameIds.length;
    const batchSize = 15; // From config
    const totalBatches = Math.ceil(totalGames / batchSize);
    
    // Show initial progress
    updateProgressBar(0, `Starting refresh of ${totalGames} games...`);
    
    try {
        const result = await invoke('smart_refresh_library', { 
            appIds: gameIds 
        });
        
        // Show completion
        updateProgressBar(100, result);
    } catch (error) {
        updateProgressBar(0, `Error: ${error}`);
    }
}
```

### **Status Monitoring**
```javascript
// Check if circuit breaker is open
const stats = await invoke('get_database_stats');
console.log('Database health:', stats);
```

---

## 🎯 **Kesimpulan**

Sistem **Safe Batch Processing** memberikan solusi lengkap untuk:

1. **Menghindari Steam API ban** dengan rate limiting
2. **Efficient refresh** dengan smart priority
3. **Robust error handling** dengan circuit breaker  
4. **User-friendly monitoring** dengan progress tracking
5. **Production-ready** dengan konfigurasi optimal

**Sekarang Anda dapat safely refresh ratusan games tanpa khawatir ban dari Steam!** 🚀✨
