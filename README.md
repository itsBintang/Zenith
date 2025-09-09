# 🎮 Zenith Launcher

A modern, high-performance Steam game launcher built with Tauri and React. Features beautiful UI, intelligent caching, and seamless Steam integration.

## 📁 Project Structure

```
Zenith/
├── src/                          # Frontend React application
│   ├── components/               # React components
│   │   ├── LoadingScreen.jsx     # App initialization screen
│   │   ├── GameDetail.jsx        # Game details view
│   │   ├── MyLibrary.jsx         # Game library
│   │   ├── Catalogue.jsx         # Game search
│   │   ├── Header.jsx            # App header
│   │   ├── Sidebar.jsx           # Navigation sidebar
│   │   ├── Home.jsx              # Dashboard
│   │   ├── GameGrid.jsx          # Game grid layout
│   │   ├── FeaturedBanner.jsx    # Featured games banner
│   │   └── SkeletonLoader.jsx    # Loading components
│   ├── styles/                   # CSS stylesheets
│   │   ├── LoadingScreen.css     # Loading screen styles
│   │   ├── GameDetail.css        # Game detail styles
│   │   ├── MyLibrary.css         # Library styles
│   │   └── SkeletonLoader.css    # Loading skeleton styles
│   ├── assets/                   # Static assets
│   │   └── zenith.svg            # App logo
│   ├── App.jsx                   # Main application component
│   ├── App.css                   # Global styles
│   └── main.jsx                  # Application entry point
├── src-tauri/                    # Backend Rust application
│   ├── src/
│   │   └── main.rs               # Tauri backend with Steam API integration
│   ├── icons/                    # Application icons
│   ├── Cargo.toml               # Rust dependencies
│   └── tauri.conf.json          # Tauri configuration
├── public/                       # Public assets
│   └── vite.svg                 # Vite logo
├── package.json                  # Node.js dependencies
├── vite.config.js               # Vite configuration
└── README.md                    # This file
```

## 🛠️ Requirements

### System Requirements
- **Windows 10/11** (primary platform)
- **Steam** installed and configured
- **Internet connection** for game data fetching

### Development Requirements

#### Frontend
- **Node.js** `>=18.0.0`
- **npm** `>=9.0.0` or **yarn** `>=1.22.0`

#### Backend
- **Rust** `>=1.70.0`
- **Cargo** (included with Rust)

#### Additional Tools
- **Tauri CLI**: `npm install -g @tauri-apps/cli`

## 🚀 Getting Started

### 1. Clone Repository
```bash
git clone <repository-url>
cd Zenith
```

### 2. Install Dependencies
```bash
# Install Node.js dependencies
npm install

# Rust dependencies will be installed automatically
```

### 3. Development Mode
```bash
# Start development server with hot reload
npm run tauri dev
```

### 4. Build for Production
```bash
# Create production build with installers
npm run tauri build
```

## 📦 Build Output

Production builds generate:
- **MSI Installer**: `src-tauri/target/release/bundle/msi/Zenith Launcher_x.x.x_x64_en-US.msi`
- **NSIS Installer**: `src-tauri/target/release/bundle/nsis/Zenith Launcher_x.x.x_x64-setup.exe`

## ⚡ Features

- **🚀 Fast Loading**: Intelligent caching with 7-day game name cache
- **🔄 Parallel Processing**: Concurrent Steam API requests with rate limiting
- **💫 Smooth UX**: Beautiful loading screens and animations
- **🎯 Smart Search**: Direct AppID search and name-based search
- **📚 Library Management**: Easy game installation and removal
- **🛡️ Error Handling**: Graceful fallbacks and user feedback

## 🔧 Architecture

- **Frontend**: React + Vite for modern web development
- **Backend**: Rust + Tauri for native performance
- **API Integration**: Steam Web API with intelligent caching
- **State Management**: React hooks with Tauri commands
- **Styling**: Modern CSS with gradients and animations

---

Built with ❤️ using Tauri + React
