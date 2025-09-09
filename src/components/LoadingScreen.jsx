import React from 'react';
import '../styles/LoadingScreen.css';

function LoadingScreen({ progress = 0, currentStep = 'Initializing...', isComplete = false, error = null }) {
  return (
    <div className="loading-screen">
      <div className="loading-container">
        {/* Logo/Brand */}
        <div className="loading-logo">
          <h1 className="logo-text">ZENITH</h1>
        </div>

        {/* Progress Bar */}
        <div className="loading-progress">
          <div className="progress-bar">
            <div 
              className="progress-fill" 
              style={{ width: `${progress}%` }}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

export default LoadingScreen;
