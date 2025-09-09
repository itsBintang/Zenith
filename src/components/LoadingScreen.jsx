import React from 'react';
import '../styles/LoadingScreen.css';

function LoadingScreen({ progress = 0, currentStep = 'Initializing...', isComplete = false, error = null }) {
  const steps = [
    'Initializing Zenith...',
    'Checking Steam installation...',
    'Initializing cache system...',
    'Loading game library...',
    'Pre-loading games...',
    'Finalizing setup...'
  ];

  const getStepStatus = (index) => {
    const stepProgress = (progress / 100) * steps.length;
    if (stepProgress > index + 1) return 'completed';
    if (stepProgress > index) return 'active';
    return 'pending';
  };

  return (
    <div className="loading-screen">
      <div className="loading-container">
        {/* Logo/Brand */}
        <div className="loading-logo">
          <img src="/zenith.svg" alt="Zenith" className="logo-image" />
          <h1 className="logo-text">ZENITH</h1>
          <p className="logo-subtitle">Gaming Launcher</p>
        </div>

        {/* Progress Bar */}
        <div className="loading-progress">
          <div className="progress-bar">
            <div 
              className="progress-fill" 
              style={{ width: `${progress}%` }}
            />
          </div>
          <div className="progress-text">
            <span className="progress-percentage">{Math.round(progress)}%</span>
            <span className="progress-step">{currentStep}</span>
          </div>
        </div>

        {/* Steps Indicator */}
        <div className="loading-steps">
          {steps.map((step, index) => (
            <div key={index} className={`step ${getStepStatus(index)}`}>
              <div className="step-indicator">
                {getStepStatus(index) === 'completed' && (
                  <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor">
                    <path d="M10 3L4.5 8.5L2 6" stroke="currentColor" strokeWidth="2" fill="none"/>
                  </svg>
                )}
                {getStepStatus(index) === 'active' && (
                  <div className="step-spinner" />
                )}
                {getStepStatus(index) === 'pending' && (
                  <div className="step-dot" />
                )}
              </div>
              <span className="step-text">{step}</span>
            </div>
          ))}
        </div>

        {/* Error Message */}
        {error && (
          <div className="loading-error">
            <div className="error-icon">⚠️</div>
            <div className="error-message">
              <h3>Initialization Error</h3>
              <p>{error}</p>
              <small>The app will continue loading in a moment...</small>
            </div>
          </div>
        )}

        {/* Loading Animation */}
        {!error && !isComplete && (
          <div className="loading-animation">
            <div className="loading-dots">
              <div className="dot"></div>
              <div className="dot"></div>
              <div className="dot"></div>
            </div>
          </div>
        )}

        {/* Success Animation */}
        {isComplete && !error && (
          <div className="loading-success">
            <div className="success-icon">✓</div>
            <p>Ready to launch!</p>
          </div>
        )}
      </div>
    </div>
  );
}

export default LoadingScreen;
