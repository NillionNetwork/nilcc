use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::path::Path;
use tracing::info;

#[derive(Clone)]
pub struct SqliteDb(SqlitePool);

impl SqliteDb {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let connect_options: SqliteConnectOptions = url.parse()?;
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
