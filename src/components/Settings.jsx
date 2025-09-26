import React, { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

function Settings() {
  const [downloadFolder, setDownloadFolder] = useState("");
  const [useTempZip, setUseTempZip] = useState(true);
  const [steamPath, setSteamPath] = useState("");

  useEffect(() => {
    const loadSettings = async () => {
      // Load from localStorage for download folder and temp zip
      const savedFolder = localStorage.getItem("zenith.downloadFolder") || "";
      const savedUseTemp = localStorage.getItem("zenith.useTempZip");
      setDownloadFolder(savedFolder);
      setUseTempZip(savedUseTemp === null ? true : savedUseTemp === "true");
      
      // Load Steam path from backend
      try {
        const backendSteamPath = await invoke('get_steam_path');
        if (backendSteamPath) {
          setSteamPath(backendSteamPath);
        } else {
          // Try to auto-detect if not set
          const detectedPath = await invoke('detect_steam_path');
          if (detectedPath) {
            setSteamPath(detectedPath);
            // Save the detected path
            await invoke('set_steam_path', { path: detectedPath });
          }
        }
      } catch (error) {
        console.error('Failed to load Steam path:', error);
      }
    };
    
    loadSettings();
  }, []);

  const handleBrowse = async () => {
    try {
      const selected = await open({ 
        directory: true, 
        multiple: false,
        title: "Select Download Folder"
      });
      if (selected && typeof selected === "string") {
        setDownloadFolder(selected);
        localStorage.setItem("zenith.downloadFolder", selected);
      }
    } catch (e) {
      console.error("Folder selection failed:", e);
    }
  };

  const handleToggleTemp = (e) => {
    const val = e.target.checked;
    setUseTempZip(val);
    localStorage.setItem("zenith.useTempZip", String(val));
  };

  const handleBrowseSteam = async () => {
    try {
      const selected = await open({ 
        directory: true, 
        multiple: false,
        title: "Select Steam Installation Folder"
      });
      if (selected && typeof selected === "string") {
        try {
          // Validate and save to backend
          await invoke('set_steam_path', { path: selected });
          setSteamPath(selected);
          console.log('Steam path saved successfully:', selected);
        } catch (error) {
          console.error('Failed to save Steam path:', error);
          alert(`Failed to save Steam path: ${error}`);
        }
      }
    } catch (e) {
      console.error("Steam folder selection failed:", e);
    }
  };

  return (
    <div className="ui-page">
      {/* The subheader is removed as the main Header component will handle titles */}
      <div className="ui-content">
        <div className="settings-section">
          <div className="settings-field">
            <label className="settings-label">Steam root path</label>
            <div className="settings-row">
              <input
                className="settings-input"
                type="text"
                readOnly
                value={steamPath}
                placeholder="Choose Steam installation folder..."
              />
              <button className="ui-btn" onClick={handleBrowseSteam}>Browse</button>
            </div>
          </div>

          <div className="settings-field">
            <label className="settings-label">Download folder</label>
            <div className="settings-row">
              <input
                className="settings-input"
                type="text"
                readOnly
                value={downloadFolder}
                placeholder="Choose a folder..."
              />
              <button className="ui-btn" onClick={handleBrowse}>Browse</button>
            </div>
          </div>

          <div className="settings-field">
            <label className="settings-label">Use temporary ZIP (don't save ZIP file)</label>
            <div className="settings-row">
              <input
                id="toggle-temp-zip"
                type="checkbox"
                checked={useTempZip}
                onChange={handleToggleTemp}
              />
              <label htmlFor="toggle-temp-zip" style={{ marginLeft: 8 }}>Enable</label>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default Settings;


