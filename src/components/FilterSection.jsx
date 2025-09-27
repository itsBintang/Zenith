import React, { useState, useMemo, useCallback } from 'react';
import './FilterSection.css';

/**
 * FilterSection Component
 * Displays a collapsible filter section with search and checkbox items
 * Matches Hydra's FilterSection design with virtual scrolling concept
 */
const FilterSection = ({ 
  title, 
  items, 
  color, 
  onSelect, 
  onClear,
  maxVisibleItems = 10 
}) => {
  const [search, setSearch] = useState('');
  const [isExpanded, setIsExpanded] = useState(false);

  // Filter items based on search
  const filteredItems = useMemo(() => {
    if (search.length > 0) {
      return items.filter(item =>
        item.label.toLowerCase().includes(search.toLowerCase())
      );
    }
    return items;
  }, [items, search]);

  // Count selected items
  const selectedItemsCount = useMemo(() => {
    return items.filter(item => item.checked).length;
  }, [items]);

  const onSearch = useCallback((value) => {
    setSearch(value);
  }, []);

  const handleToggleExpand = () => {
    setIsExpanded(!isExpanded);
  };

  // Show limited items when collapsed, all when expanded
  const visibleItems = isExpanded 
    ? filteredItems 
    : filteredItems.slice(0, maxVisibleItems);

  const hasMoreItems = filteredItems.length > maxVisibleItems;

  if (!items.length) {
    return null;
  }

  return (
    <div className="filter-section">
      {/* Header */}
      <div className="filter-section__header">
        <div className="filter-section__orb" />
        <h3 className="filter-section__title">{title}</h3>
      </div>

      {/* Clear button or count */}
      {selectedItemsCount > 0 ? (
        <button
          type="button"
          className="filter-section__clear-button"
          onClick={onClear}
        >
          Clear {selectedItemsCount} filter{selectedItemsCount > 1 ? 's' : ''}
        </button>
      ) : (
        <span className="filter-section__count">
          {items.length.toLocaleString()} item{items.length !== 1 ? 's' : ''}
        </span>
      )}

      {/* Search input */}
      <div className="filter-section__search">
        <input
          type="text"
          placeholder="Search..."
          value={search}
          onChange={(e) => onSearch(e.target.value)}
          className="filter-section__search-input"
        />
      </div>

      {/* Items list */}
      <div className="filter-section__items">
        {visibleItems.map((item) => (
          <div key={item.value} className="filter-section__item">
            <label className="filter-section__checkbox">
              <input
                type="checkbox"
                checked={item.checked || false}
                onChange={() => onSelect(item.value)}
                className="filter-section__checkbox-input"
              />
              <span className="filter-section__checkbox-custom"></span>
              <span className="filter-section__checkbox-label">
                {item.label}
              </span>
              {item.count && (
                <span className="filter-section__item-count">
                  ({item.count.toLocaleString()})
                </span>
              )}
            </label>
          </div>
        ))}

        {/* Show more/less button */}
        {hasMoreItems && (
          <button
            type="button"
            className="filter-section__toggle-button"
            onClick={handleToggleExpand}
          >
            {isExpanded 
              ? `Show less (${maxVisibleItems} of ${filteredItems.length})`
              : `Show all (${filteredItems.length} items)`
            }
          </button>
        )}

        {/* No results message */}
        {filteredItems.length === 0 && search.length > 0 && (
          <div className="filter-section__no-results">
            No items found for "{search}"
          </div>
        )}
      </div>
    </div>
  );
};

export default FilterSection;
