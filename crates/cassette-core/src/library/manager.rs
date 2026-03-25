use crate::library::config::ManagerConfig;
use crate::library::error::Result;
use crate::library::schema;
use crate::library::state::{FileLock, LibraryState, OperationContext};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct LibraryManager {
    pub(crate) db_pool: SqlitePool,
    pub(crate) state: Arc<RwLock<LibraryState>>,
    pub(crate) file_locks: Arc<RwLock<HashMap<PathBuf, FileLock>>>,
    pub(crate) active_operations: Arc<RwLock<HashMap<String, OperationContext>>>,
    pub(crate) config: ManagerConfig,
}

impl LibraryManager {
    pub async fn connect(database_url: &str, mut config: ManagerConfig) -> Result<Self> {
        config.database_url = database_url.to_string();

        let pool = SqlitePoolOptions::new()
            .max_connections(16)
            .connect(database_url)
            .await?;

        sqlx::query("PRAGMA journal_mode=WAL;").execute(&pool).await?;
        sqlx::query("PRAGMA foreign_keys=ON;").execute(&pool).await?;

        Self::from_pool(pool, config).await
    }

    pub async fn from_pool(pool: SqlitePool, config: ManagerConfig) -> Result<Self> {
        schema::ensure_schema_current(&pool).await?;

        let manager = Self {
            db_pool: pool,
            state: Arc::new(RwLock::new(LibraryState::Idle)),
            file_locks: Arc::new(RwLock::new(HashMap::new())),
            active_operations: Arc::new(RwLock::new(HashMap::new())),
            config,
        };

        manager.rehydrate_locks_from_db().await?;
        Ok(manager)
    }

    pub fn db_pool(&self) -> &SqlitePool {
        &self.db_pool
    }

    pub fn config(&self) -> &ManagerConfig {
        &self.config
    }

    pub async fn set_state(&self, state: LibraryState) {
        let mut guard = self.state.write().await;
        *guard = state;
    }

    pub async fn get_state(&self) -> LibraryState {
        *self.state.read().await
    }

    async fn rehydrate_locks_from_db(&self) -> Result<()> {
        let rows = sqlx::query_as::<_, (String, String, String)>(
            "SELECT file_path, locked_by, operation_id FROM file_locks",
        )
        .fetch_all(&self.db_pool)
        .await?;

        let mut in_memory = self.file_locks.write().await;
        for (file_path, locked_by, operation_id) in rows {
            if let Some(module) = crate::library::state::Module::from_db(&locked_by) {
                in_memory.insert(
                    PathBuf::from(file_path.clone()),
                    FileLock {
                        file_path: PathBuf::from(file_path),
                        locked_by: module,
                        acquired_at: chrono::Utc::now(),
                        operation_id,
                    },
                );
            }
        }

        Ok(())
    }
}
