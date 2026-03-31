pub mod config;
pub mod db;
pub mod enrich;
pub mod error;
pub mod import;
pub mod matchers;
pub mod models;
pub mod normalize;
pub mod observability;
pub mod orchestrator;
pub mod reconcile;
pub mod scanner;

pub use config::{DuplicatePolicy, LibrarianConfig, QualityConfig, ScanBehavior, ScanMode};
pub use db::LibrarianDb;
pub use error::{LibrarianError, Result};
pub use orchestrator::{
	run_librarian_sync, SyncCounts, SyncOutcome, SyncPhase, SyncStatus,
};
