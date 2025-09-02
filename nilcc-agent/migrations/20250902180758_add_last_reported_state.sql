-- Add `last_reported_event` to workloads table.

ALTER TABLE workloads ADD COLUMN last_reported_event VARCHAR(64) DEFAULT NULL;
