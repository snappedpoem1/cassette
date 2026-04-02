use anyhow::Result;
use cassette_core::{db::Db, librarian::db::LibrarianDb};
use std::path::{Path, PathBuf};

pub fn control_db_path_for_runtime(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("cassette_librarian.db")
}

pub fn open_runtime_and_control_db(db_path: &Path) -> Result<(Db, LibrarianDb)> {
    let db = Db::open(db_path)?;
    let control_db_path = control_db_path_for_runtime(db_path);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let control_db = runtime.block_on(async { LibrarianDb::connect(&control_db_path).await })?;
    Ok((db, control_db))
}
