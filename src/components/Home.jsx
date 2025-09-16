import React from "react";
import FeaturedBanner from "./FeaturedBanner";
import GameGrid from "./GameGrid";

function Home({ onGameSelect }) {
  return (
    <div className="ui-page">
      <div className="ui-subheader">
        <div className="ui-header__title">Home</div>
      </div>
      <div className="ui-content">
        <FeaturedBanner onGameSelect={onGameSelect} />
        <GameGrid onGameSelect={onGameSelect} />
      </div>
    </div>
  );
}

export default Home;


