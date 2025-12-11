-- Rename `workloads.hearbeats` to `heartbeat`

ALTER TABLE workloads RENAME COLUMN heartbeats TO heartbeat;
