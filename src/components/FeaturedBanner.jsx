import React from "react";

function FeaturedBanner({ onGameSelect }) {
  const featuredGame = {
    app_id: "2456740",
    title: "inZOI",
    description: "Every life becomes a story&quot; Create your unique story by controlling and observing the lives of 'Zois'. Customize characters and build houses using inZOI's easy-to-use tools to live the life of your dreams and experience the different emotions of life created by its deep and detailed simulation.",
    banner_image: "https://cdn.akamai.steamstatic.com/steam/apps/2456740/library_hero.jpg"
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


