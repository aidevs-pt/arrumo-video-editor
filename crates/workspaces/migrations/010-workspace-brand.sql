ALTER TABLE workspaces ADD COLUMN brand_colors_json TEXT NOT NULL DEFAULT '[]'
    CHECK (json_valid(brand_colors_json) AND json_type(brand_colors_json) = 'array');

PRAGMA user_version = 10;
