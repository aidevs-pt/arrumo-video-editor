ALTER TABLE motion_library ADD COLUMN source_run_id INTEGER;
ALTER TABLE motion_library ADD COLUMN source_result_id INTEGER;
ALTER TABLE motion_library ADD COLUMN source_motion_index INTEGER;
CREATE UNIQUE INDEX motion_library_result_origin
    ON motion_library(workspace_id, source_result_id, source_motion_index)
    WHERE source_result_id IS NOT NULL;

PRAGMA user_version = 40;
