import React from "react";

function FeaturedBanner() {
  return (
    <section className="ui-featured">
      <h2 className="ui-section-title">Featured</h2>
      <div className="ui-hero">
        <div className="ui-hero__image" />
        <div className="ui-hero__overlay">
          <h3 className="ui-hero__title">Hollow Knight: Silksong</h3>
          <p className="ui-hero__desc">
            Discover a vast, haunted kingdom in Hollow Knight: Silksong. Explore, fight and survive as you ascend to the
            peak of a land ruled by silk and song.
          </p>
        </div>
      </div>

      <div className="ui-tabs">
        <button className="ui-tab ui-tab--active">Hot now</button>
        <button className="ui-tab">Top games of the week</button>
        <button className="ui-tab">Games to beat</button>
      </div>
    </section>
  );
}

export default FeaturedBanner;


