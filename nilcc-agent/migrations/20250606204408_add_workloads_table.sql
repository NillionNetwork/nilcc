-- Create workloads table

CREATE TABLE workloads (
  id VARCHAR(36) PRIMARY KEY,
  docker_compose TEXT NOT NULL,
  environment_variables TEXT NOT NULL,
  public_container_name VARCHAR(100) NOT NULL,
  public_container_port INT NOT NULL,
  memory_mb INT NOT NULL,
  cpus INT NOT NULL,
  gpus INT NOT NULL,
  disk_gb INT NOT NULL,
  created_at DATETIME WITH TIMEZONE NOT NULL,
  updated_at DATETIME WITH TIMEZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
