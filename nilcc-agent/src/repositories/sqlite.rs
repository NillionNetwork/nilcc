use crate::repositories::{
    artifacts::{ArtifactsVersionRepository, SqliteArtifactsVersionRepository},
    workload::{SqliteWorkloadRepository, WorkloadRepository},
};
use async_trait::async_trait;
use sqlx::{
    pool::PoolConnection,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    Sqlite, SqliteConnection, SqlitePool, SqliteTransaction,
};
use std::{
    mem,
    ops::{Deref, DerefMut},
    path::Path,
    str::FromStr,
};
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct SqliteDb(pub(crate) SqlitePool);

impl SqliteDb {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let connect_options =
            SqliteConnectOptions::from_str(url)?.journal_mode(SqliteJournalMode::Wal).create_if_missing(true);
        let mut pool_options = SqlitePoolOptions::new();
        if connect_options.get_filename() == Path::new(":memory:") {
            // if we don't do this eventually the database gets dropped and tables disappear.
            pool_options = pool_options.max_lifetime(None).idle_timeout(None)
        }
        let pool = pool_options.connect_with(connect_options).await?;
        info!("Applying sqlite migrations");
        sqlx::migrate!().run(&pool).await?;
        info!("All sqlite migrations applied");
        Ok(Self(pool))
    }
}

impl From<SqliteDb> for SqlitePool {
    fn from(db: SqliteDb) -> Self {
        db.0
    }
}

#[derive(Debug, thiserror::Error)]
#[error("commit error: {0}")]
pub struct CommitError(String);

#[derive(Debug)]
pub(crate) enum SqliteTransactionContextInner<'a> {
    Transaction(SqliteTransaction<'a>),
    Connection(PoolConnection<Sqlite>),
}

#[derive(Debug)]
pub struct SqliteTransactionContext<'a> {
    inner: Option<SqliteTransactionContextInner<'a>>,
}

impl<'a> From<SqliteTransactionContextInner<'a>> for SqliteTransactionContext<'a> {
    fn from(inner: SqliteTransactionContextInner<'a>) -> Self {
        Self { inner: Some(inner) }
    }
}

impl SqliteTransactionContext<'_> {
    /// Commit this transaction.
    pub async fn commit(mut self) -> Result<(), sqlx::Error> {
        match mem::take(&mut self.inner).unwrap() {
            SqliteTransactionContextInner::Transaction(tx) => tx.commit().await,
            SqliteTransactionContextInner::Connection(_) => Ok(()),
        }
    }
}

impl<'a> Deref for SqliteTransactionContext<'a> {
    type Target = SqliteConnection;

    fn deref(&self) -> &Self::Target {
        match self.inner.as_ref().unwrap() {
            SqliteTransactionContextInner::Transaction(tx) => tx,
            SqliteTransactionContextInner::Connection(connection) => connection,
        }
    }
}

impl<'a> DerefMut for SqliteTransactionContext<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self.inner.as_mut().unwrap() {
            SqliteTransactionContextInner::Transaction(tx) => tx,
            SqliteTransactionContextInner::Connection(connection) => connection,
        }
    }
}

impl Drop for SqliteTransactionContext<'_> {
    fn drop(&mut self) {
        if matches!(self.inner, Some(SqliteTransactionContextInner::Transaction(_))) {
            warn!("Transaction was not committed");
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("failed to create repository: {0}")]
pub struct ProviderError(String);

/// A provider for repositories.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait RepositoryProvider: Send + Sync {
    async fn workloads(&self, mode: ProviderMode) -> Result<Box<dyn WorkloadRepository>, ProviderError>;
    async fn artifacts_version(&self, mode: ProviderMode)
        -> Result<Box<dyn ArtifactsVersionRepository>, ProviderError>;
}

pub struct SqliteRepositoryProvider {
    db: SqliteDb,
}

impl SqliteRepositoryProvider {
    pub fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    async fn build_ctx(&self, mode: ProviderMode) -> Result<SqliteTransactionContext<'static>, ProviderError> {
        let ctx = match mode {
            ProviderMode::Single => {
                let connection = self.db.0.acquire().await.map_err(|e| ProviderError(e.to_string()))?;
                SqliteTransactionContextInner::Connection(connection)
            }
            ProviderMode::Transactional => {
                let tx = self.db.0.begin().await.map_err(|e| ProviderError(e.to_string()))?;
                SqliteTransactionContextInner::Transaction(tx)
            }
        };
        Ok(ctx.into())
    }
}

#[async_trait]
impl RepositoryProvider for SqliteRepositoryProvider {
    async fn workloads(&self, mode: ProviderMode) -> Result<Box<dyn WorkloadRepository>, ProviderError> {
        let ctx = self.build_ctx(mode).await?;
        Ok(Box::new(SqliteWorkloadRepository::new(ctx)))
    }

    async fn artifacts_version(
        &self,
        mode: ProviderMode,
    ) -> Result<Box<dyn ArtifactsVersionRepository>, ProviderError> {
        let ctx = self.build_ctx(mode).await?;
        Ok(Box::new(SqliteArtifactsVersionRepository::new(ctx)))
    }
}

#[derive(Debug, Default)]
pub enum ProviderMode {
    #[default]
    Single,
    Transactional,
}
