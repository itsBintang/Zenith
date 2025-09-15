import React, { useState, useEffect } from 'react';
import { FiUser, FiUpload, FiEdit, FiAward, FiBarChart2, FiGitMerge } from 'react-icons/fi';
import { invoke } from '@tauri-apps/api/core';
import LibraryGrid from './LibraryGrid';
import logoImage from "../../logo.jpg";
import '../styles/UserProfile.css';

// Placeholder banner image
const bannerImage = "https://images.unsplash.com/photo-1579546929518-9e396f3cc809?ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxwaG90by1wYWdlfHx8fGVufDB8fHx8fA%3D%3D&auto=format&fit=crop&w=2070&q=80";


function UserProfile({ onGameSelect, onBack, libraryState, onRefreshLibrary, onUpdateFilter }) {
  const [profile, setProfile] = useState(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isUploading, setIsUploading] = useState(false);
  const [bannerImage64, setBannerImage64] = useState(null);
  const [avatarImage64, setAvatarImage64] = useState(null);
  
  const totalPlaytimeMinutes = libraryState.games.reduce((acc, game) => acc + (game.playtime_forever || 0), 0);
  const totalPlaytimeHours = (totalPlaytimeMinutes / 60).toFixed(1);

  // Load profile data on component mount
  useEffect(() => {
    loadProfile();
  }, []);

  const loadProfile = async () => {
    try {
      setIsLoading(true);
      const profileData = await invoke('get_user_profile');
      setProfile(profileData);
      
      // Load banner and avatar images as base64
      try {
        const bannerBase64 = await invoke('get_profile_image_base64', { imageType: 'banner' });
        setBannerImage64(bannerBase64);
        
        const avatarBase64 = await invoke('get_profile_image_base64', { imageType: 'avatar' });
        setAvatarImage64(avatarBase64);
      } catch (imageError) {
        console.error('Failed to load profile images:', imageError);
      }
    } catch (error) {
      console.error('Failed to load profile:', error);
    } finally {
      setIsLoading(false);
    }
  };

  const handleBannerUpload = async () => {
    try {
      setIsUploading(true);
      
      // Create file input element
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = 'image/*';
      
      input.onchange = async (event) => {
        const file = event.target.files[0];
        if (!file) return;
        
        try {
          // Convert file to array buffer
          const arrayBuffer = await file.arrayBuffer();
          const uint8Array = new Uint8Array(arrayBuffer);
          
          // Upload to backend
          const imagePath = await invoke('upload_profile_image', {
            imageData: Array.from(uint8Array),
            imageType: 'banner'
          });
          
          // Reload profile to get updated data
          await loadProfile();
          
          console.log('Banner uploaded successfully:', imagePath);
        } catch (error) {
          console.error('Failed to upload banner:', error);
          alert('Failed to upload banner. Please try again.');
        } finally {
          setIsUploading(false);
        }
      };
      
      input.click();
    } catch (error) {
      console.error('Error opening file dialog:', error);
      setIsUploading(false);
    }
  };

  if (isLoading) {
    return (
      <div className="profile-page-container">
        <div style={{ display: 'flex', justifyContent: 'center', alignItems: 'center', height: '100vh' }}>
          <div>Loading profile...</div>
        </div>
      </div>
    );
  }

  return (
    <div className="profile-page-container">
      {/* Profile Header */}
      <header className="profile-header">
        <div className="profile-banner">
          <img 
            src={bannerImage64 || bannerImage} 
            alt="User Banner" 
          />
          <div className="profile-banner-actions">
            <button 
              className="profile-action-btn" 
              onClick={handleBannerUpload}
              disabled={isUploading}
            >
              <FiUpload size={16} />
              <span>{isUploading ? 'Uploading...' : 'Upload Banner'}</span>
            </button>
          </div>
        </div>
        <div className="profile-details-bar">
          <div className="profile-avatar-section">
            <img 
              src={avatarImage64 || logoImage} 
              alt={profile?.name || 'User'} 
              className="profile-avatar" 
            />
            <div className="profile-user-info">
              <div className="profile-user-name-badge">
                <h1 className="profile-user-name">{profile?.name || 'Nazril'}</h1>
                <span className="profile-user-badge">PREMIUM</span>
              </div>
            </div>
          </div>
          <div className="profile-main-actions">
             <button className="profile-action-btn edit-profile">
                <FiEdit size={16} />
                <span>Edit Profile</span>
              </button>
          </div>
        </div>
      </header>
      
      {/* Main Content */}
      <main className="profile-main-content">
        {/* Library Section */}
        <div className="profile-library-column">
          <LibraryGrid
            onGameSelect={onGameSelect}
            title="Library"
            showHeader={true}
            showFilter={false}
            showRefresh={false} // Refresh is handled globally
            gridView={true}
            libraryState={libraryState}
            onRefreshLibrary={onRefreshLibrary}
            onUpdateFilter={onUpdateFilter}
          />
        </div>
      </main>
    </div>
  );
}

export default UserProfile;
