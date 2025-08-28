use crate::repositories::sqlite::SqliteTransactionContext;
use async_trait::async_trait;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ArtifactsVersionRepository: Send + Sync {
    /// Set the current artifacts version.
    async fn set(&mut self, version: &str) -> Result<(), ArtifactsVersionRepositoryError>;

    /// Get the current artifacts version, if any
    async fn get(&mut self) -> Result<Option<String>, ArtifactsVersionRepositoryError>;
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
        let query = "UPDATE artifacts_version SET version = ?";
        let result = sqlx::query(query).bind(version).execute(&mut *self.ctx).await?;
        if result.rows_affected() == 1 {
            return Ok(());
        }
        let query = "INSERT INTO artifacts_version (version) VALUES (?)";
        sqlx::query(query).bind(version).execute(&mut *self.ctx).await?;
        Ok(())
    }

    async fn get(&mut self) -> Result<Option<String>, ArtifactsVersionRepositoryError> {
        let query = "SELECT version FROM artifacts_version";
        let version: Option<(String,)> = sqlx::query_as(query).fetch_optional(&mut *self.ctx).await?;
        Ok(version.map(|v| v.0))
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
    }
}
