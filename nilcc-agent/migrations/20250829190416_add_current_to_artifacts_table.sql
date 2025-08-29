-- Add a `current` column to the artifacts version table.

ALTER TABLE artifacts_version RENAME TO artifacts_version_old;

CREATE TABLE artifacts_version(
  version VARCHAR(64) PRIMARY KEY,
  updated_at DATETIME WITH TIMEZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
  current BOOLEAN DEFAULT false
);

INSERT INTO artifacts_version(version, updated_at) SELECT * FROM artifacts_version_old;

DROP TABLE artifacts_version_old;
