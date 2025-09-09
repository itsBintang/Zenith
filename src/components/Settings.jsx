import React, { useEffect, useState } from "react";
import { open } from "@tauri-apps/api/dialog";

function Settings() {
  const [downloadFolder, setDownloadFolder] = useState("");
  const [useTempZip, setUseTempZip] = useState(true);

  useEffect(() => {
    const savedFolder = localStorage.getItem("zenith.downloadFolder") || "";
    const savedUseTemp = localStorage.getItem("zenith.useTempZip");
    setDownloadFolder(savedFolder);
    setUseTempZip(savedUseTemp === null ? true : savedUseTemp === "true");
  }, []);

  const handleBrowse = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
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

  return (
    <div className="ui-page">
      <div className="ui-subheader">
        <div className="ui-header__title">Settings</div>
      </div>
      <div className="ui-content">
        <div className="settings-section">
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


