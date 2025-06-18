use crate::repositories::sqlite::SqliteDb;
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{prelude::FromRow, SqlitePool};
use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    num::NonZeroU16,
};
use strum::{Display, EnumString};
use uuid::Uuid;

#[derive(FromRow, Clone, PartialEq)]
pub struct WorkloadModel {
    pub id: Uuid,
    pub docker_compose: String,
    #[sqlx(json)]
    pub environment_variables: HashMap<String, String>,
    pub public_container_name: String,
    pub public_container_port: u16,
    pub memory_mb: u32,
    pub cpus: NonZeroU16,
    pub disk_gb: NonZeroU16,
    pub gpus: u16,
    pub metal_http_port: u16,
    pub metal_https_port: u16,
    pub status: WorkloadModelStatus,
}

impl fmt::Debug for WorkloadModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            id,
            docker_compose,
            environment_variables,
            public_container_name,
            public_container_port,
            memory_mb,
            cpus,
            disk_gb,
            gpus,
            metal_http_port,
            metal_https_port,
            status,
        } = self;
        // Hide this one since it can have sensitive data
        let environment_variables: BTreeMap<_, _> = environment_variables.keys().map(|key| (key, "...")).collect();
        f.debug_struct("Workload")
            .field("id", id)
            .field("docker_compose", docker_compose)
            .field("env_vars", &environment_variables)
            .field("public_container_name", public_container_name)
            .field("public_container_port", public_container_port)
            .field("memory_mb", memory_mb)
            .field("cpus", cpus)
            .field("disk_gb", disk_gb)
            .field("gpus", gpus)
            .field("metal_http_port", metal_http_port)
            .field("metal_https_port", metal_https_port)
            .field("status", status)
            .finish()
    }
}

impl From<WorkloadModel> for crate::data_schemas::Workload {
    fn from(workload: WorkloadModel) -> Self {
        let WorkloadModel {
            id,
            docker_compose,
            environment_variables: env_vars,
            public_container_name: service_to_expose,
            public_container_port: service_port_to_expose,
            memory_mb,
            cpus: cpu,
            disk_gb: disk,
            gpus: gpu,
            ..
        } = workload;
        let memory = memory_mb / 1024; // Convert Mb to Gb
        Self {
            id,
            docker_compose,
            env_vars,
            service_to_expose,
            service_port_to_expose,
            memory_gb: memory,
            cpus: cpu,
            disk_space_gb: disk,
            gpus: gpu,
        }
    }
}

impl WorkloadModel {
    pub fn from_schema(workload: crate::data_schemas::Workload, metal_http_port: u16, metal_https_port: u16) -> Self {
        let crate::data_schemas::Workload {
            id,
            docker_compose,
            env_vars,
            service_to_expose,
            service_port_to_expose,
            memory_gb: memory,
            cpus: cpu,
            disk_space_gb: disk,
            gpus: gpu,
        } = workload;
        Self {
            id,
            docker_compose,
            environment_variables: env_vars,
            public_container_name: service_to_expose,
            public_container_port: service_port_to_expose,
            memory_mb: memory * 1024, // Convert Gb to Mb
            cpus: cpu,
            disk_gb: disk,
            gpus: gpu,
            metal_http_port,
            metal_https_port,
            status: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Display, EnumString, sqlx::Type)]
pub enum WorkloadModelStatus {
    #[default]
    Pending,
    Running,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait WorkloadRepository: Send + Sync {
    /// Create a workload.
    async fn upsert(&self, workload: WorkloadModel) -> Result<(), WorkloadRepositoryError>;

    /// Find the details for a workload.
    async fn find(&self, id: Uuid) -> Result<WorkloadModel, WorkloadRepositoryError>;

    /// List all workflows.
    async fn list(&self) -> Result<Vec<WorkloadModel>, WorkloadRepositoryError>;

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
    async fn upsert(&self, workload: WorkloadModel) -> Result<(), WorkloadRepositoryError> {
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
    metal_http_port,
    metal_https_port,
    status,
    created_at
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
ON CONFLICT (id) DO UPDATE SET
    docker_compose = $2,
    environment_variables = $3,
    public_container_name = $4,
    public_container_port = $5,
    memory_mb = $6,
    cpus = $7,
    gpus = $8,
    disk_gb = $9,
    metal_http_port = $10,
    metal_https_port = $11,
    status = $12
";
        let WorkloadModel {
            id,
            docker_compose,
            environment_variables,
            public_container_name,
            public_container_port,
            memory_mb,
            cpus,
            disk_gb,
            gpus,
            metal_http_port,
            metal_https_port,
            status,
        } = workload;

        sqlx::query(query)
            .bind(id)
            .bind(docker_compose)
            .bind(sqlx::types::Json(environment_variables))
            .bind(public_container_name)
            .bind(public_container_port)
            .bind(memory_mb)
            .bind(cpus)
            .bind(gpus)
            .bind(disk_gb)
            .bind(metal_http_port)
            .bind(metal_https_port)
            .bind(status.to_string())
            .bind(Utc::now())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn find(&self, id: Uuid) -> Result<WorkloadModel, WorkloadRepositoryError> {
        let query = "SELECT * FROM workloads WHERE id = ?";
        let workload: WorkloadModel = sqlx::query_as(query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(WorkloadRepositoryError::WorkloadNotFound)?;
        Ok(workload)
    }

    async fn list(&self) -> Result<Vec<WorkloadModel>, WorkloadRepositoryError> {
        let query = "SELECT * FROM workloads";
        let workloads: Vec<WorkloadModel> = sqlx::query_as(query).fetch_all(&self.pool).await?;
        Ok(workloads)
    }

    async fn delete(&self, id: Uuid) -> Result<(), WorkloadRepositoryError> {
        let query = "DELETE FROM workloads WHERE id = ?";
        sqlx::query(query).bind(id).execute(&self.pool).await?;
        Ok(())
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
        let workload = WorkloadModel {
            id: Uuid::new_v4(),
            docker_compose: "hi".into(),
            environment_variables: HashMap::from([("FOO".into(), "value".into())]),
            public_container_name: "container-1".into(),
            public_container_port: 80,
            memory_mb: 1024,
            cpus: 1.try_into().unwrap(),
            disk_gb: 10.try_into().unwrap(),
            gpus: 1,
            metal_http_port: 1080,
            metal_https_port: 1443,
            status: WorkloadModelStatus::Running,
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
        let original = WorkloadModel {
            id: Uuid::new_v4(),
            docker_compose: "hi".into(),
            environment_variables: HashMap::from([("FOO".into(), "value".into())]),
            public_container_name: "container-1".into(),
            public_container_port: 80,
            memory_mb: 1024,
            cpus: 1.try_into().unwrap(),
            disk_gb: 10.try_into().unwrap(),
            gpus: 1,
            metal_http_port: 1080,
            metal_https_port: 1443,
            status: WorkloadModelStatus::Pending,
        };
        let updated = WorkloadModel {
            id: original.id,
            docker_compose: "bye".into(),
            environment_variables: HashMap::default(),
            public_container_name: "container-2".into(),
            public_container_port: 443,
            memory_mb: 2048,
            gpus: 2.try_into().unwrap(),
            disk_gb: 20.try_into().unwrap(),
            cpus: 2.try_into().unwrap(),
            metal_http_port: 1080,
            metal_https_port: 1443,
            status: WorkloadModelStatus::Running,
        };
        repo.upsert(original).await.expect("failed to insert");
        repo.upsert(updated.clone()).await.expect("failed to insert");

        let found = repo.find(updated.id).await.expect("failed to find");
        assert_eq!(found, updated);
    }
}
