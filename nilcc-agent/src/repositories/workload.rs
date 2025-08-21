use crate::{repositories::sqlite::SqliteTransactionContext, resources::GpuAddress};
use async_trait::async_trait;
use chrono::Utc;
use nilcc_agent_models::workloads::create::DockerCredentials;
use sqlx::prelude::FromRow;
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
    #[sqlx(json)]
    pub docker_credentials: Vec<DockerCredentials>,
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
            docker_credentials,
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
            .field("docker_credentials", docker_credentials)
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
    async fn create(&mut self, workload: Workload) -> Result<(), WorkloadRepositoryError>;

    /// Find the details for a workload.
    async fn find(&mut self, id: Uuid) -> Result<Workload, WorkloadRepositoryError>;

    /// List all workflows.
    async fn list(&mut self) -> Result<Vec<Workload>, WorkloadRepositoryError>;

    /// Delete a workload.
    async fn delete(&mut self, id: Uuid) -> Result<(), WorkloadRepositoryError>;

    /// Set the `enabled` column for a workload.
    async fn set_enabled(&mut self, id: Uuid, value: bool) -> Result<(), WorkloadRepositoryError>;

    /// Set the `gpus` column for a workload.
    async fn set_gpus(&mut self, id: Uuid, gpus: Vec<GpuAddress>) -> Result<(), WorkloadRepositoryError>;

    /// Commit any changes that were performed on this repository.
    async fn commit(self: Box<Self>) -> Result<(), WorkloadRepositoryError>;
}

#[derive(Debug, thiserror::Error)]
pub enum WorkloadRepositoryError {
    #[error("workload not found")]
    WorkloadNotFound,

    #[error("workload already exists")]
    DuplicateWorkload,

    #[error("domain is already managed by another workload")]
    DuplicateDomain,

    #[error("database error: {0}")]
    Database(sqlx::Error),
}

impl From<sqlx::Error> for WorkloadRepositoryError {
    fn from(e: sqlx::Error) -> Self {
        match &e {
            sqlx::Error::Database(inner) if inner.is_unique_violation() => {
                match inner.code().as_deref() {
                    // SQLITE_CONSTRAINT_PRIMARYKEY
                    Some("1555") => Self::DuplicateWorkload,
                    // SQLITE_CONSTRAINT_UNIQUE
                    // Note: this is assuming there's only a single unique constraint
                    Some("2067") => Self::DuplicateDomain,
                    _ => Self::Database(e),
                }
            }
            _ => Self::Database(e),
        }
    }
}

pub struct SqliteWorkloadRepository<'a> {
    ctx: SqliteTransactionContext<'a>,
}

impl<'a> SqliteWorkloadRepository<'a> {
    pub fn new(ctx: SqliteTransactionContext<'a>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl<'a> WorkloadRepository for SqliteWorkloadRepository<'a> {
    async fn create(&mut self, workload: Workload) -> Result<(), WorkloadRepositoryError> {
        let query = r"
INSERT INTO workloads (
    id,
    docker_compose,
    env_vars,
    files,
    docker_credentials,
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
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
";
        let Workload {
            id,
            docker_compose,
            env_vars,
            files,
            docker_credentials,
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
            .bind(sqlx::types::Json(docker_credentials))
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
            .execute(&mut *self.ctx)
            .await?;
        Ok(())
    }

    async fn find(&mut self, id: Uuid) -> Result<Workload, WorkloadRepositoryError> {
        let query = "SELECT * FROM workloads WHERE id = ?";
        let workload: Workload = sqlx::query_as(query)
            .bind(id)
            .fetch_optional(&mut *self.ctx)
            .await?
            .ok_or(WorkloadRepositoryError::WorkloadNotFound)?;
        Ok(workload)
    }

    async fn list(&mut self) -> Result<Vec<Workload>, WorkloadRepositoryError> {
        let query = "SELECT * FROM workloads";
        let workloads: Vec<Workload> = sqlx::query_as(query).fetch_all(&mut *self.ctx).await?;
        Ok(workloads)
    }

    async fn delete(&mut self, id: Uuid) -> Result<(), WorkloadRepositoryError> {
        let query = "DELETE FROM workloads WHERE id = ?";
        sqlx::query(query).bind(id).execute(&mut *self.ctx).await?;
        Ok(())
    }

    async fn set_enabled(&mut self, id: Uuid, value: bool) -> Result<(), WorkloadRepositoryError> {
        let query = "UPDATE workloads SET enabled = ? WHERE id = ?";
        sqlx::query(query).bind(value).bind(id).execute(&mut *self.ctx).await?;
        Ok(())
    }

    async fn set_gpus(&mut self, id: Uuid, gpus: Vec<GpuAddress>) -> Result<(), WorkloadRepositoryError> {
        let query = "UPDATE workloads SET gpus = ? WHERE id = ?";
        sqlx::query(query).bind(sqlx::types::Json(gpus)).bind(id).execute(&mut *self.ctx).await?;
        Ok(())
    }

    async fn commit(self: Box<Self>) -> Result<(), WorkloadRepositoryError> {
        Ok(self.ctx.commit().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::sqlite::{SqliteDb, SqliteTransactionContextInner};
    use std::collections::HashMap;

    #[tokio::test]
    async fn crud() {
        let db = SqliteDb::connect("sqlite://:memory:").await.expect("failed to create db");
        let connection = db.0.acquire().await.expect("failed to acquire");
        let mut repo = SqliteWorkloadRepository::new(SqliteTransactionContextInner::Connection(connection).into());
        let workload = Workload {
            id: Uuid::new_v4(),
            docker_compose: "hi".into(),
            env_vars: HashMap::from([("FOO".into(), "value".into())]),
            files: HashMap::from([("foo.txt".into(), vec![1, 2, 3])]),
            docker_credentials: vec![DockerCredentials {
                server: "registry.example.com".into(),
                username: "foo".into(),
                password: "bar".into(),
            }],
            public_container_name: "container-1".into(),
            public_container_port: 80,
            memory_mb: 1024,
            cpus: 1.try_into().unwrap(),
            disk_space_gb: 10.try_into().unwrap(),
            gpus: vec!["aa:bb".into()],
            ports: [1080, 1443, 2000],
            domain: "example.com".into(),
            enabled: true,
        };
        repo.create(workload.clone()).await.expect("failed to insert");

        let workload_same_id = Workload { domain: "other.com".into(), ..workload.clone() };
        let err = repo.create(workload_same_id).await.expect_err("insertion succeeded");
        assert!(matches!(err, WorkloadRepositoryError::DuplicateWorkload), "{err:?}");

        let found = repo.find(workload.id).await.expect("failed to find");
        assert_eq!(found, workload);

        let found = repo.list().await.expect("failed to find");
        assert_eq!(found, &[workload.clone()]);

        repo.set_enabled(workload.id, false).await.expect("failed to update");
        assert_eq!(repo.find(workload.id).await.expect("failed to find").enabled, false);

        repo.set_gpus(workload.id, vec!["cc:dd".into()]).await.expect("failed to update");
        assert_eq!(repo.find(workload.id).await.expect("failed to find").gpus, vec!["cc:dd".into()]);

        let workload_same_domain = Workload { id: Uuid::new_v4(), ..workload };
        let err = repo.create(workload_same_domain.clone()).await.expect_err("insertion succeeded");
        assert!(matches!(err, WorkloadRepositoryError::DuplicateDomain), "{err:?}");
    }
}
