ALTER TABLE completed_media ADD COLUMN variant_sha256 BLOB
    CHECK (variant_sha256 IS NULL OR length(variant_sha256) = 32);

CREATE UNIQUE INDEX completed_media_variant_identity
    ON completed_media(run_id, format_id, variant_sha256)
    WHERE variant_sha256 IS NOT NULL;

PRAGMA user_version = 48;
