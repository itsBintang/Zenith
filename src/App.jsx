import React from "react";
import { invoke } from "@tauri-apps/api/tauri";
import Sidebar from "./components/Sidebar";
import Home from "./components/Home";
import Catalogue from "./components/Catalogue";
import GameDetail from "./components/GameDetail";
import LoadingScreen from "./components/LoadingScreen";
import "./App.css";

function App() {
  const [route, setRoute] = React.useState("home");
  const [activeAppId, setActiveAppId] = React.useState(null);
  const [isLoading, setIsLoading] = React.useState(true);
  const [loadingProgress, setLoadingProgress] = React.useState(0);
  const [loadingStep, setLoadingStep] = React.useState("Initializing Zenith...");
  const [initError, setInitError] = React.useState(null);

  // App initialization
  React.useEffect(() => {
    const initializeApp = async () => {
      try {
        setLoadingStep("Starting initialization...");
        setLoadingProgress(10);
        
        // Small delay for smooth UX
        await new Promise(resolve => setTimeout(resolve, 500));
        
        const progressSteps = await invoke('initialize_app');
        
        // Animate through progress steps
        for (let i = 0; i < progressSteps.length; i++) {
          const step = progressSteps[i];
          setLoadingStep(step.step);
          setLoadingProgress(step.progress);
          
          // Small delay between steps for smooth animation
          await new Promise(resolve => setTimeout(resolve, 300));
        }
        
        // Final completion
        setLoadingStep("Welcome to Zenith!");
        setLoadingProgress(100);
        await new Promise(resolve => setTimeout(resolve, 800));
        
        setIsLoading(false);
        
      } catch (error) {
        console.error('App initialization failed:', error);
        setInitError(error.toString());
        setLoadingStep("Initialization failed");
        // Still allow app to continue after 3 seconds
        setTimeout(() => {
          setIsLoading(false);
        }, 3000);
      }
    };

    initializeApp();
  }, []);

  React.useEffect(() => {
    const handler = (e) => {
      const { appId } = e.detail || {};
      if (appId) {
        setActiveAppId(appId);
        setRoute("detail");
      }
    };
    window.addEventListener('open-game-detail', handler);
    return () => window.removeEventListener('open-game-detail', handler);
  }, []);

  const handleGameSelect = (appId) => {
    setActiveAppId(appId);
    setRoute("detail");
  };

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
      content = <Home />;
      break;
    case 'catalogue':
      content = <Catalogue onGameSelect={handleGameSelect} />;
      break;
    case 'detail':
      content = <GameDetail appId={activeAppId} onBack={() => setRoute('catalogue')} />;
      break;
    default:
      content = <Home />;
  }

  return (
    <div className="ui-shell">
      <Sidebar 
        active={route} 
        onNavigate={setRoute} 
        onGameSelect={handleGameSelect} 
      />
      <main className="ui-main">
        {content}
      </main>
    </div>
  );
}

export default App;
