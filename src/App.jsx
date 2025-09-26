import React from "react";
import { Outlet, useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import CustomTitleBar from "./components/CustomTitleBar";
import Sidebar from "./components/Sidebar";
import LoadingScreen from "./components/LoadingScreen";
import Header from "./components/Header"; // Import the new Header
import "./App.css";

function App() {
  const [isLoading, setIsLoading] = React.useState(true);
  const [loadingProgress, setLoadingProgress] = React.useState(0);
  const [loadingStep, setLoadingStep] = React.useState("Initializing Zenith...");
  const [initError, setInitError] = React.useState(null);
  const [toastNotification, setToastNotification] = React.useState(null);
  
  // We'll need a way to manage library and profile state later, 
  // perhaps with Context API as a replacement for the old prop drilling.
  const [libraryState, setLibraryState] = React.useState({ games: [], isLoading: true, error: null, filter: '' });
  const [profileRefreshTrigger, setProfileRefreshTrigger] = React.useState(0);
  const [globalSearchQuery, setGlobalSearchQuery] = React.useState("");
  
  const navigate = useNavigate();

  React.useEffect(() => {
    let isMounted = true;
    const initializeApp = async () => {
      try {
        setLoadingStep("Starting initialization...");
        await new Promise(resolve => setTimeout(resolve, 500));
        if (!isMounted) return;
        
        const progressSteps = await invoke('initialize_app');
        if (!isMounted) return;

        for (let i = 0; i < progressSteps.length; i++) {
          const step = progressSteps[i];
          if (!isMounted) return;
          setLoadingStep(step.step);
          setLoadingProgress(step.progress);
          await new Promise(resolve => setTimeout(resolve, 300));
        }
        
        if (!isMounted) return;
        
        setLoadingStep("Welcome to Zenith!");
        setLoadingProgress(100);
        await new Promise(resolve => setTimeout(resolve, 800));
        
        if (isMounted) setIsLoading(false);
        
      } catch (error) {
        console.error('App initialization failed:', error);
        if (isMounted) {
          setInitError(error.toString());
          setLoadingStep("Initialization failed");
          setTimeout(() => {
            if (isMounted) setIsLoading(false);
          }, 3000);
        }
      }
    };

    initializeApp();
    
    return () => { isMounted = false; };
  }, []);

  React.useEffect(() => {
    if (!isLoading && initError && initError.includes('Steam installation not found')) {
      setToastNotification({
        type: 'error',
        message: 'Steam has not been detected. Please install Steam first, then restart Zenith.'
      });
    }
  }, [isLoading, initError]);

  React.useEffect(() => {
    if (!toastNotification) return;

    const timer = setTimeout(() => {
      setToastNotification(null);
    }, 6000);

    return () => clearTimeout(timer);
  }, [toastNotification]);

  const closeToastNotification = React.useCallback(() => {
    setToastNotification(null);
  }, []);

  // Event listener to handle navigation from backend or other non-React parts
  React.useEffect(() => {
    const handler = (e) => {
      const { appId } = e.detail || {};
      if (appId) {
        navigate(`/game/${appId}`);
      }
    };
    window.addEventListener('open-game-detail', handler);
    return () => window.removeEventListener('open-game-detail', handler);
  }, [navigate]);

  const loadLibrary = async () => {
    setLibraryState(prev => ({ ...prev, isLoading: true, error: null }));
    try {
      const games = await invoke('get_library_games');
      setLibraryState(prev => ({ ...prev, games: games || [], isLoading: false }));
    } catch (error) {
      console.error('Error loading library:', error);
      setLibraryState(prev => ({ ...prev, error: error.message || 'Failed to load library', isLoading: false }));
    }
  };

  React.useEffect(() => {
    if (!isLoading && !initError) {
      loadLibrary();
    }
  }, [isLoading, initError]);


  if (isLoading) {
    return (
      <div className="app-container">
        <CustomTitleBar theme="app-theme" />
        <LoadingScreen 
          progress={loadingProgress}
          currentStep={loadingStep}
          isComplete={loadingProgress >= 100}
          error={initError}
        />
      </div>
    );
  }

  return (
    <div className="app-container">
      <CustomTitleBar theme="app-theme" />
      <div className="ui-shell">
        <Sidebar 
          // Pass necessary state and functions to Sidebar
          // Note: navigation is now handled by <Link> or useNavigate in the component itself
          libraryState={libraryState}
          onRefreshLibrary={loadLibrary}
          onUpdateFilter={(filter) => setLibraryState(prev => ({ ...prev, filter }))}
          refreshProfileTrigger={profileRefreshTrigger}
        />
        <div className="ui-main-container">
          <Header 
            globalSearchQuery={globalSearchQuery}
            setGlobalSearchQuery={setGlobalSearchQuery}
          />
          <main className="ui-main">
            {/* Outlet will render the matched child route component (Home, Catalogue, etc.) */}
            <Outlet context={{ 
                // We can pass down state and functions via Outlet's context
                // to avoid prop drilling through intermediate routes.
                libraryState, 
                refreshLibrary: loadLibrary,
                updateLibraryFilter: (filter) => setLibraryState(prev => ({ ...prev, filter })),
                refreshProfile: () => setProfileRefreshTrigger(p => p + 1),
                globalSearchQuery,
                setGlobalSearchQuery,
            }}/>
          </main>
        </div>
      </div>

      {toastNotification && (
        <div className="toast-notification-overlay">
          <div className={`toast-notification ${toastNotification.type}`}>
            <div className="toast-content">
              <div className="toast-icon">
                {toastNotification.type === 'success' ? '✓' : '✗'}
              </div>
              <div className="toast-message">{toastNotification.message}</div>
              <button className="toast-close" onClick={closeToastNotification}>
                ×
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
