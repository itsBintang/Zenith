import React from "react";
import { invoke } from "@tauri-apps/api/core";
import Sidebar from "./components/Sidebar";
import Home from "./components/Home";
import Catalogue from "./components/Catalogue";
import GameDetail from "./components/GameDetail";
import LoadingScreen from "./components/LoadingScreen";
import Settings from "./components/Settings";
import UserProfile from "./components/UserProfile";
import "./App.css";

function App() {
  const [route, setRoute] = React.useState("home");
  const [activeAppId, setActiveAppId] = React.useState(null);
  const [fromLibrary, setFromLibrary] = React.useState(false);
  const [catalogueState, setCatalogueState] = React.useState({
    query: "",
    results: [],
    hasSearched: false
  });
  const [isLoading, setIsLoading] = React.useState(true);
  const [loadingProgress, setLoadingProgress] = React.useState(0);
  const [loadingStep, setLoadingStep] = React.useState("Initializing Zenith...");
  const [initError, setInitError] = React.useState(null);
  
  // Shared library state
  const [libraryState, setLibraryState] = React.useState({
    games: [],
    isLoading: true,
    error: null,
    filter: ''
  });

  // Profile refresh state
  const [profileRefreshTrigger, setProfileRefreshTrigger] = React.useState(0);

  // App initialization
  React.useEffect(() => {
    let isMounted = true;
    let hasInitialized = false;
    
    const initializeApp = async () => {
      // Prevent double initialization in React.StrictMode
      if (hasInitialized) return;
      hasInitialized = true;
      
      try {
        setLoadingStep("Starting initialization...");
        setLoadingProgress(10);
        
        // Small delay for smooth UX
        await new Promise(resolve => setTimeout(resolve, 500));
        
        if (!isMounted) return;
        
        const progressSteps = await invoke('initialize_app');
        
        if (!isMounted) return;
        
        // Animate through progress steps
        for (let i = 0; i < progressSteps.length; i++) {
          const step = progressSteps[i];
          if (!isMounted) return;
          setLoadingStep(step.step);
          setLoadingProgress(step.progress);
          
          // Small delay between steps for smooth animation
          await new Promise(resolve => setTimeout(resolve, 300));
        }
        
        if (!isMounted) return;
        
        // Final completion
        setLoadingStep("Welcome to Zenith!");
        setLoadingProgress(100);
        await new Promise(resolve => setTimeout(resolve, 800));
        
        if (isMounted) {
          setIsLoading(false);
        }
        
      } catch (error) {
        console.error('App initialization failed:', error);
        if (isMounted) {
          setInitError(error.toString());
          setLoadingStep("Initialization failed");
          // Still allow app to continue after 3 seconds
          setTimeout(() => {
            if (isMounted) setIsLoading(false);
          }, 3000);
        }
      }
    };

    initializeApp();
    
    return () => {
      isMounted = false;
    };
  }, []);

  React.useEffect(() => {
    const handler = (e) => {
      const { appId } = e.detail || {};
      if (appId) {
        setActiveAppId(appId);
        setFromLibrary(false); // Reset when coming from event
        setRoute("detail");
      }
    };
    window.addEventListener('open-game-detail', handler);
    return () => window.removeEventListener('open-game-detail', handler);
  }, []);

  // Reset fromLibrary when navigating away from detail
  React.useEffect(() => {
    if (route !== 'detail') {
      setFromLibrary(false);
    }
  }, [route]);

  const handleGameSelect = (appId) => {
    setActiveAppId(appId);
    setFromLibrary(false);
    setRoute("detail");
  };

  const handleLibraryGameSelect = (appId) => {
    setActiveAppId(appId);
    setFromLibrary(true);
    setRoute("detail");
  };

  const handleProfileClick = () => {
    setRoute("profile");
  };

  // Shared library functions
  const loadLibrary = async () => {
    setLibraryState(prev => ({ ...prev, isLoading: true, error: null }));
    
    try {
      const games = await invoke('get_library_games');
      setLibraryState(prev => ({ 
        ...prev, 
        games: games || [], 
        isLoading: false 
      }));
    } catch (error) {
      console.error('Error loading library:', error);
      setLibraryState(prev => ({ 
        ...prev, 
        error: error.message || 'Failed to load library', 
        isLoading: false 
      }));
    }
  };

  const refreshLibrary = async () => {
    await loadLibrary();
  };

  const updateLibraryFilter = (filter) => {
    setLibraryState(prev => ({ ...prev, filter }));
  };

  // Profile refresh function
  const refreshProfile = () => {
    setProfileRefreshTrigger(prev => prev + 1);
  };

  // Load library on app initialization
  React.useEffect(() => {
    if (!isLoading && !initError) {
      loadLibrary();
    }
  }, [isLoading, initError]);

  // Show loading screen during initialization
  if (isLoading) {
    return (
      <LoadingScreen 
        progress={loadingProgress}
        currentStep={loadingStep}
        isComplete={loadingProgress >= 100}
        error={initError}
      />
    );
  }

  let content;
  switch (route) {
    case 'home':
      content = <Home onGameSelect={handleGameSelect} />;
      break;
    case 'catalogue':
      content = <Catalogue 
        onGameSelect={handleGameSelect} 
        catalogueState={catalogueState}
        setCatalogueState={setCatalogueState}
      />;
      break;
    case 'detail':
      content = <GameDetail 
        appId={activeAppId} 
        onBack={() => setRoute('catalogue')} 
        showBackButton={!fromLibrary}
      />;
      break;
    case 'settings':
      content = <Settings />;
      break;
    case 'profile':
      content = <UserProfile 
        onGameSelect={handleLibraryGameSelect}
        onBack={() => setRoute('home')}
        libraryState={libraryState}
        onRefreshLibrary={refreshLibrary}
        onUpdateFilter={updateLibraryFilter}
        onProfileUpdate={refreshProfile}
      />;
      break;
    default:
      content = <Home />;
  }

  return (
    <div className="ui-shell">
      <Sidebar 
        active={route} 
        onNavigate={setRoute} 
        onGameSelect={handleLibraryGameSelect}
        onProfileClick={handleProfileClick}
        libraryState={libraryState}
        onRefreshLibrary={refreshLibrary}
        onUpdateFilter={updateLibraryFilter}
        refreshProfileTrigger={profileRefreshTrigger}
      />
      <main className="ui-main">
        {content}
      </main>
    </div>
  );
}

export default App;
