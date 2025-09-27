import React from 'react';
import './FilterItem.css';

/**
 * FilterItem Component
 * Displays an active filter badge with colored orb and remove button
 * Matches Hydra's FilterItem design
 */
const FilterItem = ({ filter, orbColor, onRemove }) => {
  return (
    <div className="filter-item">
      <div className="filter-item__orb" />
      <span className="filter-item__label">{filter}</span>
      <button
        type="button"
        onClick={onRemove}
        className="filter-item__remove-button"
        aria-label={`Remove ${filter} filter`}
      >
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <path
            d="M1 1L12 12M12 1L1 12"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </button>
    </div>
  );
};

export default FilterItem;
