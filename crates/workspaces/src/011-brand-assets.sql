CREATE UNIQUE INDEX workspace_assets_workspace_and_id ON workspace_assets(workspace_id, id);

CREATE TABLE workspace_brand_assets (
    workspace_id INTEGER NOT NULL,
    asset_id INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('identity', 'style')),
    PRIMARY KEY (workspace_id, role, asset_id),
    FOREIGN KEY (workspace_id, asset_id) REFERENCES workspace_assets(workspace_id, id) ON DELETE CASCADE
);

CREATE INDEX brand_associations_by_asset ON workspace_brand_assets(workspace_id, asset_id);

PRAGMA user_version = 11;
