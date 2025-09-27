import React from "react";
import { FaSteam } from "react-icons/fa";

// Featured games with real AppIDs
const featuredGames = [
  
  {
    app_id: "3405690",
    title: "EA SPORTS FC™ 26",
    header_image: "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/3405690/2d96aa1b06e453cd62dae9029d412f19e61932c3/header.jpg?t=1758903580"
  },
  {
    app_id: "2928600",
    title: "Demon Slayer -Kimetsu no Yaiba- The Hinokami Chronicles 2",
    header_image: "https://cdn.akamai.steamstatic.com/steam/apps/2928600/header.jpg"
  },
  {
    app_id: "1091500",
    title: "Cyberpunk 2077",
    header_image: "https://cdn.akamai.steamstatic.com/steam/apps/1091500/header.jpg"
  },
  {
    app_id: "2622380",
    title: "ELDEN RING NIGHTREIGN",
    header_image: "https://cdn.akamai.steamstatic.com/steam/apps/2622380/header.jpg"
  },
  {
    app_id: "1245620",
    title: "ELDEN RING",
    header_image: "https://cdn.akamai.steamstatic.com/steam/apps/1245620/header.jpg"
  },
  {
    app_id: "1172380",
    title: "Star Wars Jedi: Fallen Order",
    header_image: "https://cdn.akamai.steamstatic.com/steam/apps/1172380/header.jpg"
  },
];

function GameGrid({ onGameSelect }) {
  return (
    <section className="ui-section">
      <div className="ui-section__row">
        <h3 className="ui-section-sub">Hot now</h3>
      </div>
      <div className="ui-grid">
        {featuredGames.map((game) => (
          <div 
            className="ui-card ui-card--hero" 
            key={game.app_id}
            onClick={() => onGameSelect && onGameSelect(game.app_id)}
            style={{ cursor: 'pointer' }}
          >
            <div 
              className="ui-card__hero-image"
              style={{
                backgroundImage: `url(${game.header_image})`,
                backgroundSize: "cover",
                backgroundPosition: "center"
              }}
            />
            <div className="ui-card__hero-overlay">
              <div className="ui-card__hero-title-wrapper">
                <FaSteam className="ui-card__steam-icon" size={16} />
                <h3 className="ui-card__hero-title">{game.title}</h3>
              </div>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

export default GameGrid;


