-- Make the metadata column required in the `artifacts` table.

ALTER TABLE artifacts RENAME TO artifacts_old;

CREATE TABLE artifacts(
  version VARCHAR(64) PRIMARY KEY,
  updated_at DATETIME WITH TIMEZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT NOT NULL
);

INSERT INTO artifacts(version, updated_at, metadata) SELECT version, updated_at, metadata FROM artifacts_old;

DROP TABLE artifacts_old;

