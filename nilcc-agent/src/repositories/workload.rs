use crate::{repositories::sqlite::SqliteDb, resources::GpuAddress};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{prelude::FromRow, SqlitePool};
use std::{
    collections::{BTreeMap, HashMap},
    fmt,
};
use strum::{Display, EnumString};
use uuid::Uuid;

#[derive(FromRow, Clone, PartialEq)]
pub struct Workload {
    pub id: Uuid,
    pub docker_compose: String,
    #[sqlx(json)]
    pub env_vars: HashMap<String, String>,
    #[sqlx(json)]
    pub files: HashMap<String, Vec<u8>>,
    pub public_container_name: String,
    pub public_container_port: u16,
    pub memory_mb: u32,
    pub cpus: u32,
    pub enabled: bool,
    #[sqlx(json)]
    pub gpus: Vec<GpuAddress>,
    pub disk_space_gb: u32,
    #[sqlx(json)]
    pub ports: [u16; 3],
    pub domain: String,
}

impl Workload {
    pub(crate) fn http_port(&self) -> u16 {
        self.ports[0]
    }

    pub(crate) fn https_port(&self) -> u16 {
        self.ports[1]
    }

    pub(crate) fn cvm_agent_port(&self) -> u16 {
        self.ports[2]
    }
}

impl fmt::Debug for Workload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            id,
            docker_compose,
            env_vars,
            files,
            public_container_name,
            public_container_port,
            memory_mb,
            cpus,
            disk_space_gb,
            gpus,
            ports,
            domain,
            enabled: running,
        } = self;
        // Hide this one since it can have sensitive data
        let environment_variables: BTreeMap<_, _> = env_vars.keys().map(|key| (key, "...")).collect();
        f.debug_struct("Workload")
            .field("id", id)
            .field("docker_compose", docker_compose)
            .field("env_vars", &environment_variables)
            .field("files", &files)
            .field("public_container_name", public_container_name)
            .field("public_container_port", public_container_port)
            .field("memory_mb", memory_mb)
            .field("cpus", cpus)
            .field("disk_space_gb", disk_space_gb)
            .field("gpus", gpus)
            .field("ports", ports)
            .field("domain", domain)
            .field("running", running)
            .finish()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Display, EnumString, sqlx::Type)]
pub enum WorkloadModelStatus {
    #[default]
    Pending,
    Running,
    Scheduled,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait WorkloadRepository: Send + Sync {
    /// Create a workload.
    async fn create(&self, workload: Workload) -> Result<(), WorkloadRepositoryError>;

    /// Find the details for a workload.
    async fn find(&self, id: Uuid) -> Result<Workload, WorkloadRepositoryError>;

    /// List all workflows.
    async fn list(&self) -> Result<Vec<Workload>, WorkloadRepositoryError>;

    /// Delete a workload.
    async fn delete(&self, id: Uuid) -> Result<(), WorkloadRepositoryError>;

    /// Set the `enabled` column for a workload.
    async fn set_enabled(&self, id: Uuid, value: bool) -> Result<(), WorkloadRepositoryError>;
}

#[derive(Debug, thiserror::Error)]
pub enum WorkloadRepositoryError {
    #[error("workload not found")]
    WorkloadNotFound,

    #[error("workload already exists")]
    DuplicateWorkload,

    #[error("database error: {0}")]
    Database(sqlx::Error),
}

impl From<sqlx::Error> for WorkloadRepositoryError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::Database(e) if e.is_unique_violation() => Self::DuplicateWorkload,
            _ => Self::Database(e),
        }
    }
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
    env_vars,
    files,
    public_container_name,
    public_container_port,
    memory_mb,
    cpus,
    gpus,
    disk_space_gb,
    ports,
    domain,
    enabled,
    created_at
)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
";
        let Workload {
            id,
            docker_compose,
            env_vars,
            files,
            public_container_name,
            public_container_port,
            memory_mb,
            cpus,
            disk_space_gb,
            gpus,
            ports,
            domain,
            enabled,
        } = workload;

        sqlx::query(query)
            .bind(id)
            .bind(docker_compose)
            .bind(sqlx::types::Json(env_vars))
            .bind(sqlx::types::Json(files))
            .bind(public_container_name)
            .bind(public_container_port)
            .bind(memory_mb)
            .bind(cpus)
            .bind(sqlx::types::Json(gpus))
            .bind(disk_space_gb)
            .bind(sqlx::types::Json(ports))
            .bind(domain)
            .bind(enabled)
            .bind(Utc::now())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn find(&self, id: Uuid) -> Result<Workload, WorkloadRepositoryError> {
        let query = "SELECT * FROM workloads WHERE id = ?";
        let workload: Workload = sqlx::query_as(query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(WorkloadRepositoryError::WorkloadNotFound)?;
        Ok(workload)
    }

    async fn list(&self) -> Result<Vec<Workload>, WorkloadRepositoryError> {
        let query = "SELECT * FROM workloads";
        let workloads: Vec<Workload> = sqlx::query_as(query).fetch_all(&self.pool).await?;
        Ok(workloads)
    }

    async fn delete(&self, id: Uuid) -> Result<(), WorkloadRepositoryError> {
        let query = "DELETE FROM workloads WHERE id = ?";
        sqlx::query(query).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    async fn set_enabled(&self, id: Uuid, value: bool) -> Result<(), WorkloadRepositoryError> {
        let query = "UPDATE workloads SET enabled = ? WHERE id = ?";
        sqlx::query(query).bind(value).bind(id).execute(&self.pool).await?;
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
    async fn crud() {
        let repo = make_repo().await;
        let workload = Workload {
            id: Uuid::new_v4(),
            docker_compose: "hi".into(),
            env_vars: HashMap::from([("FOO".into(), "value".into())]),
            files: HashMap::from([("foo.txt".into(), vec![1, 2, 3])]),
            public_container_name: "container-1".into(),
            public_container_port: 80,
            memory_mb: 1024,
            cpus: 1.try_into().unwrap(),
            disk_space_gb: 10.try_into().unwrap(),
            gpus: vec![GpuAddress("aa:bb".into())],
            ports: [1080, 1443, 2000],
            domain: "example.com".into(),
            enabled: true,
        };
        repo.create(workload.clone()).await.expect("failed to insert");

        let found = repo.find(workload.id).await.expect("failed to find");
        assert_eq!(found, workload);

        let found = repo.list().await.expect("failed to find");
        assert_eq!(found, &[workload.clone()]);

        repo.set_enabled(workload.id, false).await.expect("failed to update");
        assert_eq!(repo.find(workload.id).await.expect("failed to find").enabled, false);
    }
}
