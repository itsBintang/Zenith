import React from "react";
import { useNavigate } from "react-router-dom";
import FeaturedBanner from "./FeaturedBanner";
import GameGrid from "./GameGrid";

function Home() {
  const navigate = useNavigate();

  const handleGameSelect = (appId) => {
    if (appId) {
      navigate(`/game/${appId}`);
    }
  };

  return (
    <div className="ui-page">
      {/* Header is now global, so local header is removed */}
      <div className="ui-content">
        <FeaturedBanner onGameSelect={handleGameSelect} />
        <GameGrid onGameSelect={handleGameSelect} />
      </div>
    </div>
  );
}

export default Home;


