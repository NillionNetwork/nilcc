-- Create workloads table

CREATE TABLE workloads (
  id VARCHAR(36) PRIMARY KEY,
  docker_compose TEXT NOT NULL,
  env_vars TEXT NOT NULL,
  files TEXT NOT NULL,
  public_container_name VARCHAR(100) NOT NULL,
  public_container_port INT NOT NULL,
  memory_mb INT NOT NULL,
  cpus INT NOT NULL,
  gpus TEXT NOT NULL,
  disk_space_gb INT NOT NULL,
  proxy_http_port INT NOT NULL,
  proxy_https_port INT NOT NULL,
  domain VARCHAR(255) NOT NULL,
  created_at DATETIME WITH TIMEZONE NOT NULL,
  updated_at DATETIME WITH TIMEZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
