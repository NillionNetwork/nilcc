-- Add a `docker_credentials` column to the `workloads` table.

ALTER TABLE workloads ADD COLUMN docker_credentials TEXT NOT NULL DEFAULT '[]';
