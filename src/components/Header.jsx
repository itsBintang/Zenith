import React from 'react';
import { useLocation, useNavigate } from "react-router-dom";
import { FiArrowLeft, FiSearch } from 'react-icons/fi';
import '../styles/Header.css';

const pathTitle = {
  '/': 'Home',
  '/catalogue': 'Catalogue',
  '/bypass': 'Bypass',
  '/settings': 'Settings',
  '/profile': 'Profile',
};

function Header({ globalSearchQuery, setGlobalSearchQuery }) {
  const navigate = useNavigate();
  const location = useLocation();

  // Function to extract App ID from Steam URL or detect pure AppID
  const extractAppIdFromUrl = (input) => {
    if (!input) return null;
    
    // Check if input is already a pure App ID (numeric)
    if (/^\d+$/.test(input.trim())) {
      return input.trim();
    }
    
    // Steam URL patterns:
    // https://store.steampowered.com/app/2622380/ELDEN_RING_NIGHTREIGN/
    // https://steamdb.info/app/578080/charts/
    // steam://store/851850
    const steamUrlPatterns = [
      /store\.steampowered\.com\/app\/(\d+)/,
      /steamdb\.info\/app\/(\d+)/,
      /steam:\/\/store\/(\d+)/
    ];
    
    for (const pattern of steamUrlPatterns) {
      const match = input.match(pattern);
      if (match) {
        return match[1];
      }
    }
    
    return null;
  };

  // A simple title logic for now. Can be expanded with context/state for dynamic titles like game names.
  const title = React.useMemo(() => {
    if (location.pathname.startsWith('/game/')) {
      // In a real app, you'd fetch the game name and set it here, maybe via context.
      // For now, we'll just show a generic title.
      return "Game Details";
    }
    return pathTitle[location.pathname] || 'Zenith';
  }, [location.pathname]);

  const canGoBack = location.key !== 'default';

  const handleBackButtonClick = () => {
    navigate(-1);
  };
  
  const handleSearchChange = (e) => {
    setGlobalSearchQuery(e.target.value);
    // Remove auto-navigation on typing, only navigate on Enter press
  };

  const handleSearchKeyPress = (e) => {
    if (e.key === 'Enter') {
      const query = e.target.value.trim();
      const extractedAppId = extractAppIdFromUrl(query);
      
      if (extractedAppId) {
        // Navigate directly to game detail page if it's a Steam URL or App ID
        navigate(`/game/${extractedAppId}`);
        return;
      }
      
      // For name search, navigate to catalogue with URL parameters
      if (query) {
        const searchParams = new URLSearchParams();
        searchParams.set('search', query);
        searchParams.set('page', '1');
        navigate(`/catalogue?${searchParams.toString()}`);
      } else {
        navigate('/catalogue');
      }
    }
  };

  return (
    <header className="header">
      <section className="header__section header__section--left">
        <button
          type="button"
          className={`header__back-button ${canGoBack ? 'header__back-button--enabled' : ''}`}
          onClick={handleBackButtonClick}
          disabled={!canGoBack}
        >
          <FiArrowLeft />
        </button>
        <h3 className="header__title">{title}</h3>
      </section>
      <section className="header__section header__section--right">
        <div className="header__search">
          <FiSearch />
          <input 
            type="text" 
            placeholder="Search games, App ID, or Steam URL..." 
            value={globalSearchQuery || ''}
            onChange={handleSearchChange}
            onKeyPress={handleSearchKeyPress}
          />
        </div>
      </section>
    </header>
  );
}

export default Header;


