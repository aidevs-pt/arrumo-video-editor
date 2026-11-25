-- How much new media one plan or preview revision may order from providers. Missing values use
-- the defaults: 12 pieces, of which 6 clips.
CREATE TABLE IF NOT EXISTS media_order_limits (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  max_media INTEGER CHECK (max_media BETWEEN 0 AND 50),
  max_clips INTEGER CHECK (max_clips BETWEEN 0 AND 50)
);

PRAGMA user_version = 70;
