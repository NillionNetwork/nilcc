use crate::{
    data_schemas::{Workload, WorkloadStatus},
    repositories::sqlite::SqliteDb,
};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait WorkloadRepository: Send + Sync {
    /// Create a workload.
    async fn upsert(&self, workload: Workload) -> Result<(), WorkloadRepositoryError>;

    /// Update a workload's status.
    async fn update_status(&self, id: Uuid, status: WorkloadStatus) -> Result<(), WorkloadRepositoryError>;

    /// Find the details for a workload.
    async fn find(&self, id: Uuid) -> Result<Workload, WorkloadRepositoryError>;

    /// List all workflows.
    async fn list(&self) -> Result<Vec<Workload>, WorkloadRepositoryError>;

    /// Delete a workload.
    async fn delete(&self, id: Uuid) -> Result<(), WorkloadRepositoryError>;
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
    async fn upsert(&self, workload: Workload) -> Result<(), WorkloadRepositoryError> {
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
    status,
    created_at
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
ON CONFLICT (id) DO UPDATE SET
    docker_compose = $2,
    environment_variables = $3,
    public_container_name = $4,
    public_container_port = $5,
    memory_mb = $6,
    cpus = $7,
    gpus = $8,
    disk_gb = $9,
    status = $10
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
            status,
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
            .bind(status.to_string())
            .bind(Utc::now())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn update_status(&self, id: Uuid, status: WorkloadStatus) -> Result<(), WorkloadRepositoryError> {
        let query = "UPDATE workloads SET status = ? WHERE id = ?";
        let result = sqlx::query(query).bind(status.to_string()).bind(id).execute(&self.pool).await?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(WorkloadRepositoryError::WorkloadNotFound)
        }
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

    async fn list(&self) -> Result<Vec<Workload>, WorkloadRepositoryError> {
        let query = "SELECT * FROM workloads";
        let workloads: Vec<model::Workload> = sqlx::query_as(query).fetch_all(&self.pool).await?;
        Ok(workloads.into_iter().map(Into::into).collect())
    }

    async fn delete(&self, id: Uuid) -> Result<(), WorkloadRepositoryError> {
        let query = "DELETE FROM workloads WHERE id = ?";
        sqlx::query(query).bind(id).execute(&self.pool).await?;
        Ok(())
    }
}

mod model {
    use crate::data_schemas::WorkloadStatus;
    use sqlx::{prelude::FromRow, types::Text};
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
        status: Text<WorkloadStatus>,
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
                status: Text(status),
            } = workload;
            Self {
                id,
                docker_compose,
                env_vars,
                service_to_expose,
                service_port_to_expose,
                memory,
                cpu,
                disk,
                gpu,
                status,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_schemas::WorkloadStatus;
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
            service_to_expose: "container-1".into(),
            service_port_to_expose: 80,
            memory: 1024,
            cpu: 1.try_into().unwrap(),
            disk: 10.try_into().unwrap(),
            gpu: 1,
            status: WorkloadStatus::Running,
        };
        repo.upsert(workload.clone()).await.expect("failed to insert");

        let found = repo.find(workload.id).await.expect("failed to find");
        assert_eq!(found, workload);

        let found = repo.list().await.expect("failed to find");
        assert_eq!(found, &[workload]);
    }

    #[tokio::test]
    async fn update() {
        let repo = make_repo().await;
        let original = Workload {
            id: Uuid::new_v4(),
            docker_compose: "hi".into(),
            env_vars: HashMap::from([("FOO".into(), "value".into())]),
            service_to_expose: "container-1".into(),
            service_port_to_expose: 80,
            memory: 1024,
            cpu: 1.try_into().unwrap(),
            disk: 10.try_into().unwrap(),
            gpu: 1,
            status: WorkloadStatus::Running,
        };
        let mut updated = Workload {
            id: original.id,
            docker_compose: "bye".into(),
            env_vars: HashMap::default(),
            service_to_expose: "container-2".into(),
            service_port_to_expose: 443,
            memory: 2048,
            cpu: 2.try_into().unwrap(),
            disk: 20.try_into().unwrap(),
            gpu: 2,
            status: WorkloadStatus::Stopped,
        };
        repo.upsert(original).await.expect("failed to insert");
        repo.upsert(updated.clone()).await.expect("failed to insert");

        let found = repo.find(updated.id).await.expect("failed to find");
        assert_eq!(found, updated);

        repo.update_status(updated.id, WorkloadStatus::Error).await.expect("failed to update status");
        let found = repo.find(updated.id).await.expect("failed to find");

        updated.status = WorkloadStatus::Error;
        assert_eq!(found, updated);
    }

    #[tokio::test]
    async fn update_unknown_status() {
        let repo = make_repo().await;
        let err = repo.update_status(Uuid::new_v4(), WorkloadStatus::Stopped).await.expect_err("update succeeded");
        assert!(matches!(err, WorkloadRepositoryError::WorkloadNotFound));
    }
}
