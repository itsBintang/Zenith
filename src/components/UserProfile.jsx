import React from 'react';
import { FiArrowLeft, FiUser, FiPlay } from 'react-icons/fi';
import LibraryGrid from './LibraryGrid';
import logoImage from "../../logo.jpg";
import '../styles/UserProfile.css';

function UserProfile({ onGameSelect, onBack, libraryState, onRefreshLibrary, onUpdateFilter }) {
  return (
    <div className="user-profile">
      {/* Header */}
      <div className="user-profile-header">
        <button className="user-profile-back-btn" onClick={onBack}>
          <FiArrowLeft size={20} />
        </button>
        
        <div className="user-profile-info">
          <img src={logoImage} alt="Nazril" className="user-profile-avatar-large" />
          <div className="user-profile-details">
            <h1 className="user-profile-name">Nazril</h1>
            <div className="user-profile-stats">
              <div className="profile-stat">
                <FiPlay size={16} />
                <span>Steam User</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Navigation Tabs */}
      <div className="user-profile-tabs">
        <button className="profile-tab profile-tab-active">
          <FiPlay size={16} />
          <span>Library</span>
        </button>
      </div>

      {/* Content */}
      <div className="user-profile-content">
        <LibraryGrid 
          onGameSelect={onGameSelect}
          title="Library"
          showHeader={true}
          showFilter={true}
          showRefresh={false}
          gridView={true}
          libraryState={libraryState}
          onRefreshLibrary={onRefreshLibrary}
          onUpdateFilter={onUpdateFilter}
        />
      </div>
    </div>
  );
}

export default UserProfile;
