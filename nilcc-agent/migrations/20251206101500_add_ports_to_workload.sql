-- Add a `metal_http_port` and `metal_https_port` columns to the `workloads` table

ALTER TABLE workloads
    ADD COLUMN metal_http_port INT NOT NULL;
ALTER TABLE workloads
    ADD COLUMN metal_https_port INT NOT NULL;