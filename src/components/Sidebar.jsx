import React from "react";
import { FiHome, FiBox, FiDownload, FiSettings, FiLogIn } from "react-icons/fi";
import MyLibrary from './MyLibrary'; // Assuming MyLibrary is in the same folder

function Sidebar({ active = "home", onNavigate, onGameSelect }) {
  return (
    <aside className="ui-sidebar">
      <div className="ui-sidebar__section">
        <button className="ui-btn ui-btn--ghost ui-btn--lg">
          <FiLogIn size={18} />
          <span>Sign in</span>
        </button>
      </div>

      <nav className="ui-sidebar__nav">
        <a className={`ui-nav-item ${active === "home" ? "ui-nav-item--active" : ""}`} onClick={() => onNavigate && onNavigate("home")}>
          <FiHome size={18} />
          <span>Home</span>
        </a>
        <a className={`ui-nav-item ${active === "catalogue" ? "ui-nav-item--active" : ""}`} onClick={() => onNavigate && onNavigate("catalogue")}>
          <FiBox size={18} />
          <span>Catalogue</span>
        </a>
        <a className={`ui-nav-item ${active === "downloads" ? "ui-nav-item--active" : ""}`} onClick={() => onNavigate && onNavigate("downloads")}>
          <FiDownload size={18} />
          <span>Downloads</span>
        </a>
        <a className={`ui-nav-item ${active === "settings" ? "ui-nav-item--active" : ""}`} onClick={() => onNavigate && onNavigate("settings")}>
          <FiSettings size={18} />
          <span>Settings</span>
        </a>
      </nav>

      <MyLibrary onGameSelect={onGameSelect} />
      
    </aside>
  );
}

export default Sidebar;


