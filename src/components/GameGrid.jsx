import React from "react";

const sample = new Array(8).fill(0).map((_, i) => ({
  id: i + 1,
  title: [
    "Hollow Knight: Silksong",
    "Red Dead Redemption 2",
    "Ghost of Tsushima",
    "Cyberpunk 2077",
  ][i % 4],
}));

function GameGrid() {
  return (
    <section className="ui-section">
      <div className="ui-section__row">
        <h3 className="ui-section-sub">Hot now</h3>
      </div>
      <div className="ui-grid">
        {sample.map((g) => (
          <div className="ui-card" key={g.id}>
            <div className="ui-card__thumb" />
            <div className="ui-card__title">{g.title}</div>
          </div>
        ))}
      </div>
    </section>
  );
}

export default GameGrid;


