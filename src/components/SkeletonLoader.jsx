import React from 'react';
import '../styles/SkeletonLoader.css';

// Generic skeleton component
export function Skeleton({ width = '100%', height = '20px', borderRadius = '4px', className = '' }) {
  return (
    <div 
      className={`skeleton ${className}`}
      style={{ 
        width, 
        height, 
        borderRadius 
      }}
    />
  );
}

// Skeleton for library game items
export function LibraryGameSkeleton() {
  return (
    <div className="library-game-skeleton">
      <Skeleton width="32px" height="15px" borderRadius="2px" />
      <Skeleton width="70%" height="14px" />
    </div>
  );
}

// Skeleton for game detail hero section
export function GameDetailHeroSkeleton() {
  return (
    <div className="game-detail-hero-skeleton">
      <div className="hero-image-skeleton">
        <Skeleton width="100%" height="100%" borderRadius="0" />
      </div>
      <div className="hero-content-skeleton">
        <div className="hero-title-skeleton">
          <Skeleton width="60%" height="32px" />
        </div>
        <div className="hero-button-skeleton">
          <Skeleton width="24px" height="24px" borderRadius="4px" />
        </div>
      </div>
    </div>
  );
}

// Skeleton for hero panel
export function HeroPanelSkeleton() {
  return (
    <div className="hero-panel-skeleton">
      <div className="hero-panel-content-skeleton">
        <Skeleton width="150px" height="16px" />
        <Skeleton width="120px" height="16px" />
      </div>
      <div className="hero-panel-actions-skeleton">
        <Skeleton width="140px" height="40px" borderRadius="8px" />
      </div>
    </div>
  );
}

// Skeleton for gallery
export function GallerySkeleton() {
  return (
    <div className="gallery-skeleton">
      <div className="gallery-main-skeleton">
        <Skeleton width="100%" height="100%" borderRadius="8px" />
      </div>
      <div className="gallery-thumbnails-skeleton">
        {[...Array(6)].map((_, index) => (
          <Skeleton 
            key={index} 
            width="100px" 
            height="56px" 
            borderRadius="4px" 
          />
        ))}
      </div>
    </div>
  );
}

// Skeleton for sidebar
export function SidebarSkeleton() {
  return (
    <div className="sidebar-skeleton">
      {/* Requirements Section */}
      <div className="sidebar-section-skeleton">
        <Skeleton width="100px" height="18px" className="section-title-skeleton" />
        <div className="requirement-buttons-skeleton">
          <Skeleton width="48%" height="32px" borderRadius="4px" />
          <Skeleton width="48%" height="32px" borderRadius="4px" />
        </div>
        <div className="requirement-content-skeleton">
          <Skeleton width="100%" height="16px" />
          <Skeleton width="90%" height="16px" />
          <Skeleton width="95%" height="16px" />
          <Skeleton width="85%" height="16px" />
        </div>
      </div>
    </div>
  );
}

// Skeleton for description content
export function DescriptionSkeleton() {
  return (
    <div className="description-skeleton">
      <div className="description-header-skeleton">
        <Skeleton width="200px" height="16px" />
      </div>
      <div className="description-content-skeleton">
        {[...Array(8)].map((_, index) => (
          <Skeleton 
            key={index} 
            width={index % 3 === 0 ? '90%' : index % 2 === 0 ? '95%' : '85%'} 
            height="16px" 
          />
        ))}
      </div>
    </div>
  );
}

// Complete game detail skeleton
export function GameDetailSkeleton() {
  return (
    <div className="game-details__wrapper">
      <section className="game-details__container">
        <GameDetailHeroSkeleton />
        <HeroPanelSkeleton />
        
        <div className="game-details__description-container">
          <div className="game-details__description-content">
            <DescriptionSkeleton />
            <GallerySkeleton />
            <div className="game-details__description">
              {[...Array(12)].map((_, index) => (
                <Skeleton 
                  key={index} 
                  width={index % 4 === 0 ? '100%' : index % 3 === 0 ? '85%' : '95%'} 
                  height="18px" 
                  className="description-line-skeleton"
                />
              ))}
            </div>
          </div>
          <SidebarSkeleton />
        </div>
      </section>
    </div>
  );
}

// Skeleton for catalogue game cards
export function CatalogueGameSkeleton() {
  return (
    <div className="catalogue-game-skeleton">
      <div className="game-image-skeleton">
        <Skeleton width="100%" height="100%" borderRadius="8px" />
      </div>
      <div className="game-info-skeleton">
        <Skeleton width="90%" height="18px" className="game-title-skeleton" />
        <Skeleton width="60%" height="14px" className="game-meta-skeleton" />
      </div>
    </div>
  );
}

// Skeleton for catalogue grid
export function CatalogueGridSkeleton({ count = 12 }) {
  return (
    <div className="catalogue-grid-skeleton">
      {[...Array(count)].map((_, index) => (
        <CatalogueGameSkeleton key={index} />
      ))}
    </div>
  );
}

// Skeleton for search results
export function SearchResultsSkeleton() {
  return (
    <div className="search-results-skeleton">
      <div className="search-header-skeleton">
        <Skeleton width="200px" height="24px" />
      </div>
      <CatalogueGridSkeleton count={8} />
    </div>
  );
}

export default Skeleton;
