use crate::repositories::sqlite::SqliteTransactionContext;
use async_trait::async_trait;
use sqlx::FromRow;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ArtifactsVersionRepository: Send + Sync {
    /// Set the current artifacts version.
    async fn set(&mut self, version: &str) -> Result<(), ArtifactsVersionRepositoryError>;

    /// Get the current artifacts version, if any
    async fn get(&mut self) -> Result<Option<String>, ArtifactsVersionRepositoryError>;

    /// List the available versions.
    async fn list(&mut self) -> Result<Vec<ArtifactVersion>, ArtifactsVersionRepositoryError>;

    /// Check if a version already exists.
    async fn exists(&mut self, version: &str) -> Result<bool, ArtifactsVersionRepositoryError>;

    /// Delete a version.
    async fn delete(&mut self, version: &str) -> Result<(), ArtifactsVersionRepositoryError>;

    /// Commit all changes.
    async fn commit(self: Box<Self>) -> Result<(), ArtifactsVersionRepositoryError>;
}

#[derive(FromRow)]
pub struct ArtifactVersion {
    pub(crate) version: String,
    pub(crate) current: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ArtifactsVersionRepositoryError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct SqliteArtifactsVersionRepository<'a> {
    ctx: SqliteTransactionContext<'a>,
}

impl<'a> SqliteArtifactsVersionRepository<'a> {
    pub fn new(ctx: SqliteTransactionContext<'a>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl<'a> ArtifactsVersionRepository for SqliteArtifactsVersionRepository<'a> {
    async fn set(&mut self, version: &str) -> Result<(), ArtifactsVersionRepositoryError> {
        let query = "UPDATE artifacts_version SET current = 0";
        sqlx::query(query).execute(&mut *self.ctx).await?;

        let query = "INSERT INTO artifacts_version (version, current) VALUES (?, 1)";
        sqlx::query(query).bind(version).execute(&mut *self.ctx).await?;
        Ok(())
    }

    async fn get(&mut self) -> Result<Option<String>, ArtifactsVersionRepositoryError> {
        let query = "SELECT version FROM artifacts_version WHERE current = 1";
        let version: Option<(String,)> = sqlx::query_as(query).fetch_optional(&mut *self.ctx).await?;
        Ok(version.map(|v| v.0))
    }

    async fn list(&mut self) -> Result<Vec<ArtifactVersion>, ArtifactsVersionRepositoryError> {
        let query = "SELECT version, current FROM artifacts_version";
        let rows = sqlx::query_as(query).fetch_all(&mut *self.ctx).await?;
        Ok(rows)
    }

    async fn exists(&mut self, version: &str) -> Result<bool, ArtifactsVersionRepositoryError> {
        let query = "SELECT 1 FROM artifacts_version WHERE version = ?";
        let row = sqlx::query(query).bind(version).fetch_optional(&mut *self.ctx).await?;
        Ok(row.is_some())
    }

    async fn delete(&mut self, version: &str) -> Result<(), ArtifactsVersionRepositoryError> {
        let query = "DELETE FROM artifacts_version WHERE version = ?";
        sqlx::query(query).bind(version).execute(&mut *self.ctx).await?;
        Ok(())
    }

    async fn commit(mut self: Box<Self>) -> Result<(), ArtifactsVersionRepositoryError> {
        self.ctx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::sqlite::{SqliteDb, SqliteTransactionContextInner};

    #[tokio::test]
    async fn crud() {
        let db = SqliteDb::connect("sqlite://:memory:").await.expect("failed to create db");
        let connection = db.0.acquire().await.expect("failed to acquire");
        let mut repo =
            SqliteArtifactsVersionRepository::new(SqliteTransactionContextInner::Connection(connection).into());

        assert!(repo.get().await.expect("failed to get").is_none());

        repo.set("aaa").await.expect("failed to set");
        assert_eq!(repo.get().await.expect("failed to get").as_deref(), Some("aaa"));

        repo.set("bbb").await.expect("failed to set");
        assert_eq!(repo.get().await.expect("failed to get").as_deref(), Some("bbb"));

        assert!(repo.exists("aaa").await.expect("lookup failed"));
        assert!(repo.exists("bbb").await.expect("lookup failed"));
        assert!(!repo.exists("cc").await.expect("lookup failed"));

        let versions = repo.list().await.expect("list failed");
        assert_eq!(versions.len(), 2);

        repo.delete("aaa").await.expect("delete failed");
        assert!(!repo.exists("aaa").await.expect("lookup failed"));
    }
}
