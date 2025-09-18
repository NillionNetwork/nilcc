use crate::repositories::sqlite::SqliteTransactionContext;
use async_trait::async_trait;
use nilcc_artifacts::metadata::ArtifactsMetadata;
use sqlx::FromRow;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ArtifactsRepository: Send + Sync {
    /// Set the current artifacts version.
    async fn set(&mut self, version: &str, metadata: &ArtifactsMetadata) -> Result<(), ArtifactsRepositoryError>;

    /// Update the metadata for an artifact.
    async fn update_metadata(
        &mut self,
        version: &str,
        metadata: &ArtifactsMetadata,
    ) -> Result<(), ArtifactsRepositoryError>;

    /// Get the current artifacts version, if any
    async fn get(&mut self) -> Result<Option<Artifacts>, ArtifactsRepositoryError>;

    /// Get the current artifacts version, if any
    async fn find(&mut self, version: &str) -> Result<Option<Artifacts>, ArtifactsRepositoryError>;

    /// List the available versions.
    async fn list(&mut self) -> Result<Vec<Artifacts>, ArtifactsRepositoryError>;

    /// Check if a version already exists.
    async fn exists(&mut self, version: &str) -> Result<bool, ArtifactsRepositoryError>;

    /// Delete a version.
    async fn delete(&mut self, version: &str) -> Result<(), ArtifactsRepositoryError>;

    /// Commit all changes.
    async fn commit(self: Box<Self>) -> Result<(), ArtifactsRepositoryError>;
}

#[derive(FromRow)]
pub struct Artifacts {
    pub version: String,
    #[sqlx(json)]
    pub metadata: Option<ArtifactsMetadata>,
    pub current: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ArtifactsRepositoryError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct SqliteArtifactsRepository<'a> {
    ctx: SqliteTransactionContext<'a>,
}

impl<'a> SqliteArtifactsRepository<'a> {
    pub fn new(ctx: SqliteTransactionContext<'a>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl<'a> ArtifactsRepository for SqliteArtifactsRepository<'a> {
    async fn set(&mut self, version: &str, metadata: &ArtifactsMetadata) -> Result<(), ArtifactsRepositoryError> {
        let query = "UPDATE artifacts SET current = 0";
        sqlx::query(query).execute(&mut *self.ctx).await?;

        let query = "INSERT INTO artifacts (version, metadata, current) VALUES (?, ?, 1)";
        sqlx::query(query).bind(version).bind(sqlx::types::Json(metadata)).execute(&mut *self.ctx).await?;
        Ok(())
    }

    async fn update_metadata(
        &mut self,
        version: &str,
        metadata: &ArtifactsMetadata,
    ) -> Result<(), ArtifactsRepositoryError> {
        let query = "UPDATE artifacts SET metadata = ? WHERE version = ?";
        sqlx::query(query).bind(sqlx::types::Json(metadata)).bind(version).execute(&mut *self.ctx).await?;
        Ok(())
    }

    async fn get(&mut self) -> Result<Option<Artifacts>, ArtifactsRepositoryError> {
        let query = "SELECT version, metadata, current FROM artifacts WHERE current = 1";
        let row = sqlx::query_as(query).fetch_optional(&mut *self.ctx).await?;
        Ok(row)
    }

    async fn find(&mut self, version: &str) -> Result<Option<Artifacts>, ArtifactsRepositoryError> {
        let query = "SELECT version, metadata, current FROM artifacts WHERE version = ?";
        let row = sqlx::query_as(query).bind(version).fetch_optional(&mut *self.ctx).await?;
        Ok(row)
    }

    async fn list(&mut self) -> Result<Vec<Artifacts>, ArtifactsRepositoryError> {
        let query = "SELECT version, metadata, current FROM artifacts";
        let rows = sqlx::query_as(query).fetch_all(&mut *self.ctx).await?;
        Ok(rows)
    }

    async fn exists(&mut self, version: &str) -> Result<bool, ArtifactsRepositoryError> {
        let query = "SELECT 1 FROM artifacts WHERE version = ?";
        let row = sqlx::query(query).bind(version).fetch_optional(&mut *self.ctx).await?;
        Ok(row.is_some())
    }

    async fn delete(&mut self, version: &str) -> Result<(), ArtifactsRepositoryError> {
        let query = "DELETE FROM artifacts WHERE version = ?";
        sqlx::query(query).bind(version).execute(&mut *self.ctx).await?;
        Ok(())
    }

    async fn commit(mut self: Box<Self>) -> Result<(), ArtifactsRepositoryError> {
        self.ctx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::sqlite::{SqliteDb, SqliteTransactionContextInner};
    use nilcc_artifacts::metadata::LegacyMetadata;

    #[tokio::test]
    async fn crud() {
        let db = SqliteDb::connect("sqlite://:memory:").await.expect("failed to create db");
        let connection = db.0.acquire().await.expect("failed to acquire");
        let mut repo = SqliteArtifactsRepository::new(SqliteTransactionContextInner::Connection(connection).into());

        assert!(repo.get().await.expect("failed to get").is_none());

        let meta = ArtifactsMetadata::legacy(LegacyMetadata {
            cpu_verity_root_hash: Default::default(),
            gpu_verity_root_hash: Default::default(),
        });
        repo.set("aaa", &meta).await.expect("failed to set");
        assert_eq!(repo.get().await.expect("failed to get").unwrap().version, "aaa");

        repo.set("bbb", &meta).await.expect("failed to set");
        assert_eq!(repo.get().await.expect("failed to get").unwrap().version, "bbb");

        assert!(repo.exists("aaa").await.expect("lookup failed"));
        assert!(repo.exists("bbb").await.expect("lookup failed"));
        assert!(!repo.exists("cc").await.expect("lookup failed"));

        let versions = repo.list().await.expect("list failed");
        assert_eq!(versions.len(), 2);

        repo.delete("aaa").await.expect("delete failed");
        assert!(!repo.exists("aaa").await.expect("lookup failed"));
    }
}
