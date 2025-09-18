-- Add a metadata field to the artifacts table.

CREATE TABLE artifacts(
  version VARCHAR(64) PRIMARY KEY,
  updated_at DATETIME WITH TIMEZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
  metadata TEXT DEFAULT NULL,
  current BOOLEAN DEFAULT false
);

INSERT INTO artifacts(version, updated_at, metadata, current) SELECT version, updated_at, 'null', current FROM artifacts_version;

DROP TABLE artifacts_version;
