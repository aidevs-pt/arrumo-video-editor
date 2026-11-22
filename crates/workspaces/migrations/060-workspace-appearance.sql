-- How a workspace is recognised in the switcher: a preset icon on a color, or one of its images.
ALTER TABLE workspaces ADD COLUMN color TEXT CHECK (color IS NULL OR (length(color) = 7 AND color GLOB '#[0-9A-F][0-9A-F][0-9A-F][0-9A-F][0-9A-F][0-9A-F]'));
ALTER TABLE workspaces ADD COLUMN icon TEXT CHECK (icon IS NULL OR (length(icon) BETWEEN 1 AND 32 AND icon NOT GLOB '*[^a-z-]*'));
ALTER TABLE workspaces ADD COLUMN image_asset_id INTEGER REFERENCES workspace_assets(id) ON DELETE SET NULL;

UPDATE workspaces SET
    color = CASE abs(random()) % 8
        WHEN 0 THEN '#E86AA8' WHEN 1 THEN '#4BA3F2' WHEN 2 THEN '#3DBB6B' WHEN 3 THEN '#E55D4D'
        WHEN 4 THEN '#C68820' WHEN 5 THEN '#138778' WHEN 6 THEN '#6974D6' ELSE '#AB5980' END,
    icon = CASE abs(random()) % 8
        WHEN 0 THEN 'clapperboard' WHEN 1 THEN 'camera' WHEN 2 THEN 'mic' WHEN 3 THEN 'sparkles'
        WHEN 4 THEN 'rocket' WHEN 5 THEN 'gamepad' WHEN 6 THEN 'book' ELSE 'palette' END;

PRAGMA user_version = 60;
