import React from "react";
import { FiSearch } from "react-icons/fi";

function Header() {
  return (
    <header className="ui-header">
      <div className="ui-header__title">Home</div>

      <div className="ui-header__actions">
        <div className="ui-input ui-input--search">
          <FiSearch size={18} />
          <input placeholder="Search games" />
        </div>
      </div>
    </header>
  );
}

export default Header;


