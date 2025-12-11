-- Add `heartbeats` to `workloads` table.

ALTER TABLE workloads ADD COLUMN heartbeats TEXT DEFAULT 'null';
ALTER TABLE workloads DROP COLUMN wallet_key;
