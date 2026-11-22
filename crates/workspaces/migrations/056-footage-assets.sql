-- Recordings imported to make videos are footage, not library assets for use inside videos.
ALTER TABLE workspace_assets ADD COLUMN footage INTEGER NOT NULL DEFAULT 0 CHECK (footage IN (0, 1));

UPDATE workspace_assets SET footage = 1
WHERE id IN (SELECT asset_id FROM video_draft_sources)
   OR id IN (SELECT source.value FROM generation_runs,
             json_each(snapshot_json, '$.draft.source_ids') AS source);

PRAGMA user_version = 56;
