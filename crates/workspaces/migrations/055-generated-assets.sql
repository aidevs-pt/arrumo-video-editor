-- Media Arrumo produced itself (animation previews and thumbnails, generated thumbnails, portrait
-- cuts, video variants) stays out of the asset library that creators fill for their videos.
ALTER TABLE workspace_assets ADD COLUMN generated INTEGER NOT NULL DEFAULT 0 CHECK (generated IN (0, 1));

UPDATE workspace_assets SET generated = 1
WHERE id IN (SELECT preview_asset_id FROM motion_library)
   OR id IN (SELECT thumbnail_asset_id FROM motion_library WHERE thumbnail_asset_id IS NOT NULL)
   OR id IN (SELECT sound_asset_id FROM motion_library WHERE sound_asset_id IS NOT NULL)
   OR id IN (SELECT json_extract(item.value, '$.preview_asset_id')
             FROM production_results, json_each(result_json, '$.motion_items') AS item)
   OR id IN (SELECT json_extract(item.value, '$.thumbnail_asset_id')
             FROM production_results, json_each(result_json, '$.motion_items') AS item)
   OR id IN (SELECT json_extract(item.value, '$.asset_id')
             FROM production_results, json_each(result_json, '$.supplemental_media') AS item)
   OR id IN (SELECT asset_id FROM completed_media WHERE variant_sha256 IS NOT NULL);

PRAGMA user_version = 55;
