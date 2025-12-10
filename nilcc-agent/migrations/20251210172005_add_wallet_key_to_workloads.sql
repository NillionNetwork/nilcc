-- Add `wallet_key` to `workloads` table.

ALTER TABLE workloads ADD COLUMN wallet_key TEXT DEFAULT 'null';
