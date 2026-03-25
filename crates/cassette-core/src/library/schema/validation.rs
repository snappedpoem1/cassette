use crate::library::error::{ManagerError, Result};
use crate::library::schema::invariants::INVARIANT_TRIGGERS;
use crate::library::schema::migrations::{MIGRATIONS, SCHEMA_VERSION};
use crate::library::types::SchemaVersion;
use sqlx::SqlitePool;

pub async fn ensure_schema_current(pool: &SqlitePool) -> Result<SchemaVersion> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at TIMESTAMP NOT NULL, migration_name TEXT NOT NULL)",
    )
    .execute(pool)
    .await?;

    let current = sqlx::query_scalar::<_, Option<i64>>("SELECT MAX(version) FROM schema_version")
        .fetch_one(pool)
        .await?
        .unwrap_or(0) as u32;

    let mut pending = Vec::new();
    for (idx, (name, _sql)) in MIGRATIONS.iter().enumerate() {
        let version = (idx + 1) as u32;
        if version > current {
            pending.push((*name).to_string());
        }
    }

    if !pending.is_empty() {
        let mut tx = pool.begin().await?;
        for (idx, (name, sql)) in MIGRATIONS.iter().enumerate() {
            let version = (idx + 1) as u32;
            if version <= current {
                continue;
            }

            sqlx::query(sql).execute(tx.as_mut()).await?;
            sqlx::query(
                "INSERT OR REPLACE INTO schema_version(version, applied_at, migration_name) VALUES(?1, CURRENT_TIMESTAMP, ?2)",
            )
            .bind(version as i64)
            .bind(*name)
            .execute(tx.as_mut())
            .await?;
        }

        for trigger_sql in INVARIANT_TRIGGERS {
            let trigger_name = sqlx::query("SELECT name FROM sqlite_master WHERE type = 'trigger' AND sql = ?1")
                .bind(*trigger_sql)
                .fetch_optional(tx.as_mut())
                .await?;
            if trigger_name.is_none() {
                sqlx::query(trigger_sql).execute(tx.as_mut()).await?;
            }
        }

        tx.commit().await?;
    }

    let current_after = sqlx::query_scalar::<_, Option<i64>>("SELECT MAX(version) FROM schema_version")
        .fetch_one(pool)
        .await?
        .unwrap_or(0) as u32;

    if current_after > SCHEMA_VERSION {
        return Err(ManagerError::SchemaMismatch {
            expected: SCHEMA_VERSION,
            current: current_after,
        });
    }

    let required_tables = [
        "schema_version",
        "operation_log",
        "operation_events",
        "file_locks",
        "invariant_violations",
    ];
    for table in required_tables {
        let exists = sqlx::query("SELECT 1 FROM sqlite_master WHERE type='table' AND name = ?1")
            .bind(table)
            .fetch_optional(pool)
            .await?
            .is_some();
        if !exists {
            return Err(ManagerError::ConfigError(format!(
                "required table missing after migration: {table}"
            )));
        }
    }

    let incompatible = current_after != SCHEMA_VERSION;
    Ok(SchemaVersion {
        current: current_after,
        expected: SCHEMA_VERSION,
        is_compatible: !incompatible,
        pending_migrations: Vec::new(),
    })
}
