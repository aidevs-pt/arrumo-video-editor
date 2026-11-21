-- Animations Arrumo rendered from an agent's spec keep it, so runs can render them at any size.
ALTER TABLE motion_library ADD COLUMN spec_json TEXT;

PRAGMA user_version = 54;
