use crate::gatekeeper::batch::summary::summarize;
use crate::gatekeeper::config::GatekeeperConfig;
use crate::gatekeeper::error::Result;
use crate::gatekeeper::mod_types::{BatchIngestOutcome, IngressOutcome};
use crate::gatekeeper::orchestrator::ingest_single_file;
use std::path::PathBuf;

pub async fn ingest_files(
    db_pool: &sqlx::SqlitePool,
    config: &GatekeeperConfig,
    files: Vec<PathBuf>,
) -> Result<BatchIngestOutcome> {
    let mut outcomes: Vec<IngressOutcome> = Vec::with_capacity(files.len());
    for file in files {
        let outcome = ingest_single_file(db_pool, config, &file, None).await?;
        outcomes.push(outcome);
    }
    Ok(summarize(outcomes))
}
