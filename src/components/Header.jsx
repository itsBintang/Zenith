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
    if (location.pathname !== '/catalogue') {
      navigate('/catalogue');
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
            placeholder="Search games..." 
            value={globalSearchQuery || ''}
            onChange={handleSearchChange}
          />
        </div>
      </section>
    </header>
  );
}

export default Header;


