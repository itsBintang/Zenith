import React, { useState, useEffect } from 'react';
import { FiX, FiUser, FiImage, FiUpload, FiSave } from 'react-icons/fi';
import { invoke } from '@tauri-apps/api/core';
import '../styles/EditProfileModal.css';

function EditProfileModal({ isOpen, onClose, profile, onProfileUpdate }) {
  const [formData, setFormData] = useState({
    name: '',
    bio: ''
  });
  const [isUploading, setIsUploading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [errors, setErrors] = useState({});
  const [previewImages, setPreviewImages] = useState({
    banner: null,
    avatar: null
  });

  // Initialize form data when profile changes
  useEffect(() => {
    if (profile) {
      setFormData({
        name: profile.name || '',
        bio: profile.bio || '' // Bio can be empty string or null, both are valid
      });
    }
  }, [profile]);

  // Load current images for preview
  useEffect(() => {
    if (isOpen) {
      loadImagePreviews();
    }
  }, [isOpen]);

  const loadImagePreviews = async () => {
    try {
      const bannerBase64 = await invoke('get_profile_image_base64', { imageType: 'banner' });
      const avatarBase64 = await invoke('get_profile_image_base64', { imageType: 'avatar' });
      
      setPreviewImages({
        banner: bannerBase64,
        avatar: avatarBase64
      });
    } catch (error) {
      console.error('Failed to load image previews:', error);
    }
  };

  const handleInputChange = (field, value) => {
    setFormData(prev => ({
      ...prev,
      [field]: value
    }));
    
    // Clear error when user starts typing
    if (errors[field]) {
      setErrors(prev => ({
        ...prev,
        [field]: null
      }));
    }
  };

  const validateForm = () => {
    const newErrors = {};
    
    if (!formData.name.trim()) {
      newErrors.name = 'Name is required';
    } else if (formData.name.trim().length < 2) {
      newErrors.name = 'Name must be at least 2 characters';
    }
    
    if (formData.bio && formData.bio.length > 200) {
      newErrors.bio = 'Bio must be less than 200 characters';
    }
    
    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleImageUpload = async (imageType) => {
    if (isUploading) {
      console.log('Upload already in progress, ignoring request');
      return;
    }
    
    setIsUploading(true);
    
    // Fail-safe: reset uploading state after 60 seconds no matter what
    const failsafeTimeout = setTimeout(() => {
      console.warn('Upload fail-safe triggered - resetting upload state');
      setIsUploading(false);
    }, 60000);
    
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
        
        // Validate file size (max 5MB)
        if (file.size > 5 * 1024 * 1024) {
          alert('Image size must be less than 5MB');
          setIsUploading(false);
          return;
        }
        
        // Validate file type
        if (!file.type.startsWith('image/')) {
          alert('Please select a valid image file');
          setIsUploading(false);
          return;
        }
        
        try {
          console.log(`Starting ${imageType} upload, file size:`, file.size);
          
          // Convert file to array buffer
          const arrayBuffer = await file.arrayBuffer();
          const uint8Array = new Uint8Array(arrayBuffer);
          
          console.log(`Converted to array buffer, uploading ${imageType}...`);
          
          // Upload to backend with timeout
          const uploadPromise = invoke('upload_profile_image', {
            imageData: Array.from(uint8Array),
            imageType: imageType
          });
          
          // 30 second timeout
          const timeoutPromise = new Promise((_, reject) => 
            setTimeout(() => reject(new Error('Upload timeout after 30 seconds')), 30000)
          );
          
          const imagePath = await Promise.race([uploadPromise, timeoutPromise]);
          console.log(`${imageType} uploaded successfully:`, imagePath);
          
          // Update preview
          console.log(`Loading updated ${imageType} preview...`);
          const newBase64 = await invoke('get_profile_image_base64', { imageType: imageType });
          setPreviewImages(prev => ({
            ...prev,
            [imageType]: newBase64
          }));
          
          console.log(`${imageType} upload completed successfully`);
        } catch (error) {
          console.error(`Failed to upload ${imageType}:`, error);
          
          // Show more specific error messages
          let errorMessage = `Failed to upload ${imageType}.`;
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
          clearTimeout(failsafeTimeout);
          setIsUploading(false);
        }
      };
      
      input.click();
    } catch (error) {
      console.error('Error opening file dialog:', error);
      clearTimeout(failsafeTimeout);
      setIsUploading(false);
    }
  };

  const handleSave = async () => {
    if (!validateForm()) {
      return;
    }
    
    setIsSaving(true);
    
    try {
      console.log('Saving profile with data:', formData);
      
      // Update each field individually
      await invoke('update_profile_field', { field: 'name', value: formData.name.trim() });
      console.log('Name updated successfully');
      
      // Always update bio, even if it's empty (this allows clearing the bio)
      const bioValue = formData.bio.trim() === '' ? null : formData.bio.trim();
      console.log('Updating bio with value:', bioValue);
      await invoke('update_profile_field', { field: 'bio', value: bioValue });
      console.log('Bio updated successfully');
      
      // Notify parent component to refresh profile
      if (onProfileUpdate) {
        await onProfileUpdate();
      }
      
      console.log('Profile save completed');
      onClose();
    } catch (error) {
      console.error('Failed to save profile:', error);
      alert(`Failed to save profile: ${error.message || error}`);
    } finally {
      setIsSaving(false);
    }
  };

  const handleClose = () => {
    if (!isSaving && !isUploading) {
      onClose();
    }
  };

  if (!isOpen) return null;

  return (
    <div className="edit-profile-modal-overlay" onClick={handleClose}>
      <div className="edit-profile-modal" onClick={e => e.stopPropagation()}>
        <div className="edit-profile-modal-header">
          <h2>Edit Profile</h2>
          <button 
            className="edit-profile-modal-close"
            onClick={handleClose}
            disabled={isSaving || isUploading}
          >
            <FiX size={20} />
          </button>
        </div>
        
        <div className="edit-profile-modal-content">
          {/* Image Upload Section */}
          <div className="edit-profile-section">
            <h3>Profile Images</h3>
            <div className="edit-profile-images">
              <div className="edit-profile-image-item">
                <label>Banner Image</label>
                <div className="edit-profile-image-preview banner-preview">
                  {previewImages.banner ? (
                    <img src={previewImages.banner} alt="Banner Preview" />
                  ) : (
                    <div className="edit-profile-image-placeholder">
                      <FiImage size={32} />
                      <span>No banner image</span>
                    </div>
                  )}
                  <button 
                    className="edit-profile-image-upload-btn"
                    onClick={() => handleImageUpload('banner')}
                    disabled={isUploading || isSaving}
                  >
                    <FiUpload size={16} />
                    <span>
                      {isUploading ? 'Uploading... Please wait' : 'Upload Banner'}
                    </span>
                  </button>
                </div>
              </div>
              
              <div className="edit-profile-image-item">
                <label>Avatar Image</label>
                <div className="edit-profile-image-preview avatar-preview">
                  {previewImages.avatar ? (
                    <img src={previewImages.avatar} alt="Avatar Preview" />
                  ) : (
                    <div className="edit-profile-image-placeholder">
                      <FiUser size={32} />
                      <span>No avatar image</span>
                    </div>
                  )}
                  <button 
                    className="edit-profile-image-upload-btn"
                    onClick={() => handleImageUpload('avatar')}
                    disabled={isUploading || isSaving}
                  >
                    <FiUpload size={16} />
                    <span>
                      {isUploading ? 'Uploading... Please wait' : 'Upload Avatar'}
                    </span>
                  </button>
                </div>
              </div>
            </div>
          </div>
          
          {/* Profile Information Section */}
          <div className="edit-profile-section">
            <h3>Profile Information</h3>
            <div className="edit-profile-form">
              <div className="edit-profile-field">
                <label htmlFor="profile-name">Display Name *</label>
                <input
                  id="profile-name"
                  type="text"
                  value={formData.name}
                  onChange={(e) => handleInputChange('name', e.target.value)}
                  placeholder="Enter your display name"
                  className={errors.name ? 'error' : ''}
                  disabled={isSaving}
                  maxLength={50}
                />
                {errors.name && <span className="edit-profile-error">{errors.name}</span>}
              </div>
              
              <div className="edit-profile-field">
                <label htmlFor="profile-bio">Bio (Optional)</label>
                <textarea
                  id="profile-bio"
                  value={formData.bio}
                  onChange={(e) => handleInputChange('bio', e.target.value)}
                  placeholder="Tell others about yourself... (Leave blank if you prefer)"
                  className={errors.bio ? 'error' : ''}
                  disabled={isSaving}
                  maxLength={200}
                  rows={3}
                />
                <div className="edit-profile-char-count">
                  {formData.bio.length}/200 characters
                </div>
                {errors.bio && <span className="edit-profile-error">{errors.bio}</span>}
              </div>
              
            </div>
          </div>
        </div>
        
        <div className="edit-profile-modal-footer">
          <button 
            className="edit-profile-btn edit-profile-btn-secondary"
            onClick={handleClose}
            disabled={isSaving || isUploading}
          >
            Cancel
          </button>
          <button 
            className="edit-profile-btn edit-profile-btn-primary"
            onClick={handleSave}
            disabled={isSaving || isUploading}
          >
            <FiSave size={16} />
            <span>{isSaving ? 'Saving...' : 'Save Changes'}</span>
          </button>
        </div>
      </div>
    </div>
  );
}

export default EditProfileModal;
