-- Add artifacts version table.

CREATE TABLE artifacts_version(
  version VARCHAR(64) NOT NULL,
  updated_at DATETIME WITH TIMEZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE workloads ADD COLUMN artifacts_version TEXT NOT NULL;
