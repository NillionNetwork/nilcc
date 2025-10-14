use crate::repositories::sqlite::SqliteTransactionContext;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Decode, Sqlite, prelude::FromRow};
use strum::{Display, EnumString};
use uuid::Uuid;

#[derive(Clone, Debug, FromRow, PartialEq)]
pub struct ChangelogEntry {
    pub(crate) id: Uuid,
    pub(crate) operation: ChangelogEntryOperation,
    pub(crate) version: String,
    pub(crate) state: ChangelogEntryState,
    pub(crate) details: Option<String>,
}

#[derive(Clone, Debug, FromRow, PartialEq)]
pub struct ChangelogEntryDetails {
    #[sqlx(flatten)]
    pub(crate) entry: ChangelogEntry,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Display, EnumString, Decode, PartialEq)]
pub enum ChangelogEntryOperation {
    Install,
    Uninstall,
}

#[derive(Clone, Copy, Debug, Display, EnumString, Decode, PartialEq)]
pub enum ChangelogEntryState {
    Pending,
    Success,
    Failure,
}

impl sqlx::Type<Sqlite> for ChangelogEntryOperation {
    fn type_info() -> <Sqlite as sqlx::Database>::TypeInfo {
        <String as sqlx::Type<Sqlite>>::type_info()
    }
}

impl sqlx::Type<Sqlite> for ChangelogEntryState {
    fn type_info() -> <Sqlite as sqlx::Database>::TypeInfo {
        <String as sqlx::Type<Sqlite>>::type_info()
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ChangelogRepository: Send + Sync {
    /// Insert a new entry.
    async fn insert(&mut self, entry: &ChangelogEntry) -> Result<(), ChangelogRepositoryError>;

    /// List the changelog entries.
    async fn list(&mut self) -> Result<Vec<ChangelogEntryDetails>, ChangelogRepositoryError>;

    /// Update the state of an entry.
    async fn update_state(
        &mut self,
        id: Uuid,
        state: ChangelogEntryState,
        details: Option<String>,
    ) -> Result<(), ChangelogRepositoryError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ChangelogRepositoryError {
    #[error("duplicate changelog entry")]
    DuplicateEntry,

    #[error("database error: {0}")]
    Database(sqlx::Error),
}

impl From<sqlx::Error> for ChangelogRepositoryError {
    fn from(e: sqlx::Error) -> Self {
        match &e {
            sqlx::Error::Database(inner) if inner.is_unique_violation() => Self::DuplicateEntry,
            _ => Self::Database(e),
        }
    }
}

pub struct SqliteChangelogRepository<'a> {
    ctx: SqliteTransactionContext<'a>,
}

impl<'a> SqliteChangelogRepository<'a> {
    pub fn new(ctx: SqliteTransactionContext<'a>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl<'a> ChangelogRepository for SqliteChangelogRepository<'a> {
    async fn insert(&mut self, entry: &ChangelogEntry) -> Result<(), ChangelogRepositoryError> {
        let query = r"
INSERT INTO artifacts_changelog (id, operation, version, state, details)
VALUES ($1, $2, $3, $4, $5)";
        let ChangelogEntry { id, operation, version, state, details } = entry;
        sqlx::query(query)
            .bind(id)
            .bind(sqlx::types::Text(operation))
            .bind(version)
            .bind(sqlx::types::Text(state))
            .bind(details)
            .execute(&mut *self.ctx)
            .await?;
        Ok(())
    }

    async fn list(&mut self) -> Result<Vec<ChangelogEntryDetails>, ChangelogRepositoryError> {
        let query = "SELECT * FROM artifacts_changelog";
        let entries = sqlx::query_as(query).fetch_all(&mut *self.ctx).await?;
        Ok(entries)
    }

    async fn update_state(
        &mut self,
        id: Uuid,
        state: ChangelogEntryState,
        details: Option<String>,
    ) -> Result<(), ChangelogRepositoryError> {
        let query =
            "UPDATE artifacts_changelog SET state = $1, details = $2, updated_at = CURRENT_TIMESTAMP WHERE id = $3";
        sqlx::query(query).bind(sqlx::types::Text(state)).bind(details).bind(id).execute(&mut *self.ctx).await?;
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
        let mut repo = SqliteChangelogRepository::new(SqliteTransactionContextInner::Connection(connection).into());

        let entry = ChangelogEntry {
            id: Uuid::new_v4(),
            operation: ChangelogEntryOperation::Install,
            version: "42".into(),
            state: ChangelogEntryState::Pending,
            details: None,
        };
        repo.insert(&entry).await.expect("insert failed");

        let entries = repo.list().await.expect("list failed");
        assert_eq!(entries[0].entry, entry);
    }
}
