import React from "react";
import FeaturedBanner from "./FeaturedBanner";
import GameGrid from "./GameGrid";

function Home() {
  return (
    <div className="ui-page">
      <div className="ui-subheader">
        <div className="ui-header__title">Home</div>
        <div className="ui-header__actions">
          <div className="ui-input ui-input--search">
            <input placeholder="Search games" />
          </div>
        </div>
      </div>
      <div className="ui-content">
        <FeaturedBanner />
        <GameGrid />
      </div>
    </div>
  );
}

export default Home;


