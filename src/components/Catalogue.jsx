import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { SearchResultsSkeleton } from "./SkeletonLoader";

function Catalogue() {
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(false);
  const [results, setResults] = useState([]);
  const [error, setError] = useState("");

  const onSearch = async () => {
    if (!query.trim()) return;
    setLoading(true);
    setError("");
    try {
      const res = await invoke("search_games", { query: query.trim() });
      setResults(res || []);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const onDownload = async (item) => {
    try {
      await invoke("download_game", { appId: item.app_id, gameName: item.name });
      alert(`Started installing ${item.name}`);
    } catch (e) {
      alert(`Failed: ${e}`);
    }
  };

  return (
    <div className="ui-page">
      <div className="ui-subheader">
        <div className="ui-header__title">Catalogue</div>
        <div className="ui-header__actions">
          <div className="ui-input ui-input--search">
            <input
              placeholder="Search by AppID or name"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && onSearch()}
            />
          </div>
          <button className="ui-btn" onClick={onSearch} disabled={loading}>
            {loading ? "Searching..." : "Search"}
          </button>
        </div>
      </div>

      <div className="ui-content">
        {error && <div className="status-message">{error}</div>}
        
        {loading && <SearchResultsSkeleton />}
        
        {!loading && (
          <div className="ui-grid">
            {results.map((g) => (
              <div className="ui-card" key={g.app_id} onClick={() => window.dispatchEvent(new CustomEvent('open-game-detail', { detail: { appId: g.app_id } }))} style={{ cursor: 'pointer' }}>
                <div className="ui-card__thumb" style={{ backgroundImage: `url(${g.header_image})`, backgroundSize: "cover", backgroundPosition: "center" }} />
                <div className="ui-card__title">{g.name}</div>
                <div style={{ padding: 12 }}>
                  <button className="ui-btn" onClick={() => onDownload(g)}>Install</button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export default Catalogue;


