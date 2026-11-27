-- Canvas components: library items whose demo animation comes with a reusable module that
-- canvas pages import. Versions keep the module each earlier state had.
ALTER TABLE motion_library ADD COLUMN component_json TEXT;
ALTER TABLE motion_library_versions ADD COLUMN component_json TEXT;

PRAGMA user_version = 76;
