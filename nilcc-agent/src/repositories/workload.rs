use crate::{data_schemas::Workload, repositories::sqlite::SqliteDb};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

#[async_trait]
pub trait WorkloadRepository {
    /// Create a workload.
    async fn create(&self, workload: Workload) -> Result<(), WorkloadRepositoryError>;

    /// Find the details for a workload.
    async fn find(&self, name: Uuid) -> Result<Workload, WorkloadRepositoryError>;
}

#[derive(Debug, thiserror::Error)]
pub enum WorkloadRepositoryError {
    #[error("workload not found")]
    WorkloadNotFound,

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct SqliteWorkloadRepository {
    pool: SqlitePool,
}

impl SqliteWorkloadRepository {
    pub fn new(db: SqliteDb) -> Self {
        Self { pool: db.into() }
    }
}

#[async_trait]
impl WorkloadRepository for SqliteWorkloadRepository {
    async fn create(&self, workload: Workload) -> Result<(), WorkloadRepositoryError> {
        let query = r"
INSERT INTO workloads (
    id,
    docker_compose,
    environment_variables,
    public_container_name,
    public_container_port,
    memory_mb,
    cpus,
    gpus,
    disk_gb,
    created_at
)
VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
";
        let Workload {
            id,
            docker_compose,
            env_vars,
            service_to_expose,
            service_port_to_expose,
            memory,
            cpu,
            disk,
            gpu,
        } = workload;
        sqlx::query(query)
            .bind(id)
            .bind(docker_compose)
            .bind(sqlx::types::Json(env_vars))
            .bind(service_to_expose)
            .bind(service_port_to_expose)
            .bind(memory)
            .bind(cpu)
            .bind(gpu)
            .bind(disk)
            .bind(Utc::now())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn find(&self, id: Uuid) -> Result<Workload, WorkloadRepositoryError> {
        let query = "SELECT * FROM workloads WHERE id = ?";
        let workload: model::Workload = sqlx::query_as(query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(WorkloadRepositoryError::WorkloadNotFound)?;
        Ok(workload.into())
    }
}

mod model {
    use sqlx::prelude::FromRow;
    use std::{collections::HashMap, num::NonZeroU16};
    use uuid::Uuid;

    #[derive(FromRow)]
    pub(super) struct Workload {
        id: Uuid,
        docker_compose: String,
        #[sqlx(json)]
        environment_variables: HashMap<String, String>,
        public_container_name: String,
        public_container_port: u16,
        memory_mb: u32,
        cpus: NonZeroU16,
        disk_gb: NonZeroU16,
        gpus: u16,
    }

    impl From<Workload> for crate::data_schemas::Workload {
        fn from(workload: Workload) -> Self {
            let Workload {
                id,
                docker_compose,
                environment_variables: env_vars,
                public_container_name: service_to_expose,
                public_container_port: service_port_to_expose,
                memory_mb: memory,
                cpus: cpu,
                disk_gb: disk,
                gpus: gpu,
            } = workload;
            Self { id, docker_compose, env_vars, service_to_expose, service_port_to_expose, memory, cpu, disk, gpu }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    async fn make_repo() -> SqliteWorkloadRepository {
        let db = SqliteDb::connect("sqlite://:memory:").await.expect("failed to create db");
        SqliteWorkloadRepository::new(db)
    }

    #[tokio::test]
    async fn lookup() {
        let repo = make_repo().await;
        let workload = Workload {
            id: Uuid::new_v4(),
            docker_compose: "hi".into(),
            env_vars: HashMap::from([("FOO".into(), "value".into())]),
            service_to_expose: "contaner-1".into(),
            service_port_to_expose: 80,
            memory: 1024,
            cpu: 1.try_into().unwrap(),
            disk: 10.try_into().unwrap(),
            gpu: 1,
        };
        repo.create(workload.clone()).await.expect("failed to insert");

        let found = repo.find(workload.id).await.expect("failed to find");
        assert_eq!(found, workload);
    }
}
