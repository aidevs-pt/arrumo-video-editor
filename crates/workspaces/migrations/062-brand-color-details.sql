-- Names and video use for brand colours, keyed by the colour's #RRGGBB in brand_colors_json.
ALTER TABLE workspaces ADD COLUMN brand_color_details_json TEXT NOT NULL DEFAULT '{}'
    CHECK (json_valid(brand_color_details_json) AND json_type(brand_color_details_json) = 'object');

PRAGMA user_version = 62;
