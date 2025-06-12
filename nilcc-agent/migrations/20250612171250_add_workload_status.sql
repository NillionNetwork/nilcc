-- Add a `status` column to the `workloads` table

ALTER TABLE workloads
  ADD COLUMN status VARCHAR(64) NOT NULL;
