use crate::custodian::error::Result;
use sqlx::Row;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CustodianCandidate {
    pub id: i64,
    pub track_id: Option<i64>,
    pub file_path: PathBuf,
    pub extension: String,
}

pub async fn ensure_custodian_columns(db_pool: &sqlx::SqlitePool) -> Result<()> {
    let cols = sqlx::query("PRAGMA table_info(local_files)")
        .fetch_all(db_pool)
        .await?;

    let mut has_validation_summary = false;
    let mut has_last_processed_at = false;
    let mut has_duplicate_of_id = false;

    for row in cols {
        let name: String = row.try_get("name")?;
        match name.as_str() {
            "validation_summary" => has_validation_summary = true,
            "last_processed_at" => has_last_processed_at = true,
            "duplicate_of_id" => has_duplicate_of_id = true,
            _ => {}
        }
    }

    if !has_validation_summary {
        sqlx::query("ALTER TABLE local_files ADD COLUMN validation_summary TEXT")
            .execute(db_pool)
            .await?;
    }
    if !has_last_processed_at {
        sqlx::query("ALTER TABLE local_files ADD COLUMN last_processed_at TIMESTAMP")
            .execute(db_pool)
            .await?;
    }
    if !has_duplicate_of_id {
        sqlx::query(
            "ALTER TABLE local_files ADD COLUMN duplicate_of_id INTEGER REFERENCES local_files(id)",
        )
        .execute(db_pool)
        .await?;
    }

    Ok(())
}

fn normalize_prefix(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase()
}

fn path_is_within_roots(path: &str, roots: &[PathBuf]) -> bool {
    let normalized_path = path.replace('/', "\\").to_ascii_lowercase();
    roots.iter().any(|root| {
        let normalized_root = normalize_prefix(root);
        normalized_path == normalized_root || normalized_path.starts_with(&(normalized_root + "\\"))
    })
}

pub async fn load_candidates(
    db_pool: &sqlx::SqlitePool,
    source_roots: &[PathBuf],
) -> Result<Vec<CustodianCandidate>> {
    let rows = sqlx::query(
        "SELECT id, track_id, file_path, extension
         FROM local_files
         WHERE last_processed_at IS NULL
         ORDER BY id",
    )
    .fetch_all(db_pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let file_path: String = row.try_get("file_path")?;
        if !source_roots.is_empty() && !path_is_within_roots(&file_path, source_roots) {
            continue;
        }

        out.push(CustodianCandidate {
            id: row.try_get("id")?,
            track_id: row.try_get("track_id")?,
            file_path: PathBuf::from(file_path),
            extension: row.try_get("extension")?,
        });
    }

    Ok(out)
}

pub async fn update_local_file_after_action(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    local_file_id: i64,
    file_path: &str,
    integrity_status: &str,
    validation_summary: &str,
    duplicate_of_id: Option<i64>,
) -> Result<()> {
    sqlx::query(
        "UPDATE local_files
         SET file_path = file_path || '.stale-conflict-' || id,
             integrity_status = 'missing_on_disk',
             validation_summary = 'stale row displaced by custodian path update',
             last_processed_at = CURRENT_TIMESTAMP,
             updated_at = CURRENT_TIMESTAMP
         WHERE id != ?1
           AND LOWER(file_path) = LOWER(?2)",
    )
    .bind(local_file_id)
    .bind(file_path)
    .execute(tx.as_mut())
    .await?;

    sqlx::query(
        "UPDATE local_files
         SET file_path = ?1,
             integrity_status = ?2,
             validation_summary = ?3,
             last_processed_at = CURRENT_TIMESTAMP,
             duplicate_of_id = ?4,
             updated_at = CURRENT_TIMESTAMP
         WHERE id = ?5",
    )
    .bind(file_path)
    .bind(integrity_status)
    .bind(validation_summary)
    .bind(duplicate_of_id)
    .bind(local_file_id)
    .execute(tx.as_mut())
    .await?;

    Ok(())
}
