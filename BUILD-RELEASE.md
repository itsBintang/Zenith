# 🚀 Zenith Launcher - Build & Release Guide

## 📋 Quick Commands

### **Development**
```bash
# Run in development mode
npm run tauri dev

# Build for testing
npm run tauri build
```

### **Release Build with Auto-Updater**
```bash
# Set signing environment (run once per session)
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content zenith-private.key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""

# Build release (will auto-sign)
npm run tauri build
```

### **Automated Version Update**
```bash
# Update version and build in one command
./scripts/prepare-update.ps1 -Version "0.1.2" -Notes "New features and bug fixes"
```

## 📁 Build Output

After successful build, files are located at:
- **Installer**: `src-tauri/target/release/bundle/nsis/Zenith Launcher_X.X.X_x64-setup.exe`
- **Signature**: `src-tauri/target/release/bundle/nsis/Zenith Launcher_X.X.X_x64-setup.exe.sig`

## 🔄 Auto-Updater Setup

### **Current Status**: ✅ **ACTIVE**
- Auto-updater is fully configured and working
- Updates are served from GitHub releases
- All builds are automatically signed

### **For New Release**:
1. **Build**: `./scripts/prepare-update.ps1 -Version "X.X.X"`
2. **Commit**: `latest.json` will be created in root - commit it to repo
3. **GitHub Release**: Upload installer from `release-output/`
4. **Users get automatic updates**

## 🔐 Security

- ✅ Private key: `zenith-private.key` (secured, not in git)
- ✅ Public key: configured in `tauri.conf.json`
- ✅ All releases cryptographically signed

## 🎯 Version Management

Update versions in these files:
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

Or use the automated script: `./scripts/prepare-update.ps1`

---

**That's it! Keep it simple.** 🚀
