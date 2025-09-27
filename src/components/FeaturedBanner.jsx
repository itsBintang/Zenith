import React from "react";

function FeaturedBanner({ onGameSelect }) {
  const featuredGame = {
    app_id: "2947440",
    title: "SILENT HILL f",
    description: "Hinako's hometown is engulfed in fog, driving her to fight grotesque monsters and solve eerie puzzles. Uncover the disturbing beauty hidden in terror.",
    banner_image: "https://cdn.akamai.steamstatic.com/steam/apps/2947440/library_hero.jpg"
  };

  return (
    <section className="ui-featured">
      <div 
        className="ui-hero"
        onClick={() => onGameSelect && onGameSelect(featuredGame.app_id)}
        style={{ cursor: 'pointer' }}
      >
        <div 
          className="ui-hero__image"
          style={{
            backgroundImage: `url(${featuredGame.banner_image})`,
            backgroundSize: "cover",
            backgroundPosition: "center"
          }}
        />
        <div className="ui-hero__overlay">
          <h3 className="ui-hero__title">{featuredGame.title}</h3>
          <p className="ui-hero__desc">
            {featuredGame.description}
          </p>
        </div>
      </div>
    </section>
  );
}

export default FeaturedBanner;


