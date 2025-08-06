-- Make the `domain` column unique.

CREATE UNIQUE INDEX workload_column ON workloads (domain);
