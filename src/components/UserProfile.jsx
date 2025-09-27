import React, { useState, useEffect } from 'react';
import { useOutletContext, useNavigate } from 'react-router-dom';
import { FiUser, FiUpload, FiEdit, FiAward, FiBarChart2, FiGitMerge } from 'react-icons/fi';
import { FaCrown } from "react-icons/fa";
import { invoke } from '@tauri-apps/api/core';
import LibraryGrid from './LibraryGrid';
import EditProfileModal from './EditProfileModal';
import logoImage from "../../logo.jpg";
import '../styles/UserProfile.css';

// Placeholder banner image
const bannerImage = "https://images.unsplash.com/photo-1579546929518-9e396f3cc809?ixlib=rb-4.0.3&ixid=M3wxMjA3fDB8MHxwaG90by1wYWdlfHx8fGVufDB8fHx8fA%3D%3D&auto=format&fit=crop&w=2070&q=80";


function UserProfile() {
  const { libraryState, refreshLibrary, updateLibraryFilter, refreshProfile } = useOutletContext();
  const navigate = useNavigate();

  const [profile, setProfile] = useState(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isUploading, setIsUploading] = useState(false);
  const [bannerImage64, setBannerImage64] = useState(null);
  const [avatarImage64, setAvatarImage64] = useState(null);
  const [isEditModalOpen, setIsEditModalOpen] = useState(false);
  
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
          console.log('Starting banner upload, file size:', file.size);
          
          // Convert file to array buffer
          const arrayBuffer = await file.arrayBuffer();
          const uint8Array = new Uint8Array(arrayBuffer);
          
          console.log('Converted to array buffer, uploading banner...');
          
          // Upload to backend with timeout
          const uploadPromise = invoke('upload_profile_image', {
            imageData: Array.from(uint8Array),
            imageType: 'banner'
          });
          
          // 30 second timeout
          const timeoutPromise = new Promise((_, reject) => 
            setTimeout(() => reject(new Error('Upload timeout after 30 seconds')), 30000)
          );
          
          const imagePath = await Promise.race([uploadPromise, timeoutPromise]);
          console.log('Banner uploaded successfully:', imagePath);
          
          // Reload profile to get updated data
          console.log('Reloading profile data...');
          await loadProfile();
          
          // Refresh sidebar profile
          if (refreshProfile) {
            refreshProfile();
          }
          
          console.log('Banner upload completed successfully');
        } catch (error) {
          console.error('Failed to upload banner:', error);
          
          // Show more specific error messages
          let errorMessage = 'Failed to upload banner.';
          if (error.message) {
            if (error.message.includes('timeout')) {
              errorMessage += ' Upload timed out. Please try with a smaller image.';
            } else if (error.message.includes('too large')) {
              errorMessage += ' Image file is too large. Maximum size is 10MB.';
            } else {
              errorMessage += ` Error: ${error.message}`;
            }
          }
          errorMessage += ' Please try again.';
          
          alert(errorMessage);
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

  const handleAvatarUpload = async () => {
    setIsUploading(true);
    
    try {
      // Create file input element
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = 'image/*';
      
      input.onchange = async (event) => {
        const file = event.target.files[0];
        if (!file) {
          setIsUploading(false);
          return;
        }
        
        try {
          console.log('Starting avatar upload, file size:', file.size);
          
          // Convert file to array buffer
          const arrayBuffer = await file.arrayBuffer();
          const uint8Array = new Uint8Array(arrayBuffer);
          
          console.log('Converted to array buffer, uploading avatar...');
          
          // Upload to backend with timeout
          const uploadPromise = invoke('upload_profile_image', {
            imageData: Array.from(uint8Array),
            imageType: 'avatar'
          });
          
          // 30 second timeout
          const timeoutPromise = new Promise((_, reject) => 
            setTimeout(() => reject(new Error('Upload timeout after 30 seconds')), 30000)
          );
          
          const imagePath = await Promise.race([uploadPromise, timeoutPromise]);
          console.log('Avatar uploaded successfully:', imagePath);
          
          // Reload profile to get updated data
          console.log('Reloading profile data...');
          await loadProfile();
          
          // Refresh sidebar profile
          if (refreshProfile) {
            refreshProfile();
          }
          
          console.log('Avatar upload completed successfully');
        } catch (error) {
          console.error('Failed to upload avatar:', error);
          
          // Show more specific error messages
          let errorMessage = 'Failed to upload avatar.';
          if (error.message) {
            if (error.message.includes('timeout')) {
              errorMessage += ' Upload timed out. Please try with a smaller image.';
            } else if (error.message.includes('too large')) {
              errorMessage += ' Image file is too large. Maximum size is 10MB.';
            } else {
              errorMessage += ` Error: ${error.message}`;
            }
          }
          errorMessage += ' Please try again.';
          
          alert(errorMessage);
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

  const handleGameSelect = (appId) => {
    navigate(`/game/${appId}`);
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
            onClick={handleBannerUpload}
            style={{ cursor: 'pointer' }}
            title="Click to upload banner"
          />
        </div>
        <div className="profile-details-bar">
          <div className="profile-avatar-section">
            <div className="profile-avatar-container">
              <img 
                src={avatarImage64 || logoImage} 
                alt={profile?.name || 'User'} 
                className="profile-avatar" 
                onClick={() => handleAvatarUpload()}
                style={{ cursor: 'pointer' }}
                title="Click to upload avatar"
              />
            </div>
            <div className="profile-user-info">
              <div className="profile-user-name-badge">
                <h1 className="profile-user-name">{profile?.name || 'User'}</h1>
                <span className="profile-user-badge">
                  <FaCrown size={12} />
                  ADMIN
                </span>
              </div>
            </div>
          </div>
          <div className="profile-main-actions">
             <button 
               className="profile-action-btn edit-profile"
               onClick={() => setIsEditModalOpen(true)}
             >
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
            onGameSelect={handleGameSelect}
            title="Library"
            showHeader={true}
            showFilter={false}
            showRefresh={false} // Refresh is handled globally
            gridView={true}
            libraryState={libraryState}
            onRefreshLibrary={refreshLibrary}
            onUpdateFilter={updateLibraryFilter}
          />
        </div>
      </main>
      
      {/* Edit Profile Modal */}
      <EditProfileModal
        isOpen={isEditModalOpen}
        onClose={() => setIsEditModalOpen(false)}
        profile={profile}
        onProfileUpdate={async () => {
          await loadProfile();
          if (refreshProfile) {
            refreshProfile();
          }
        }}
      />
    </div>
  );
}

export default UserProfile;
