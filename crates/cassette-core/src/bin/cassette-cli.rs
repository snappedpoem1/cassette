use cassette_core::library::{LibraryManager, ManagerConfig};
use cassette_core::validation::logging::{get_file_lineage, get_operation_summary};
use cassette_core::validation::test_library::{sqlite_url_for_path, TestLibraryConfig};
use cassette_core::validation::{reset_validation, run_full_validation, ValidationConfig};
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "cassette")]
#[command(about = "Cassette validation and operational audit CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Validate(ValidateArgs),
    Lineage(LineageArgs),
    Operation(OperationArgs),
}

#[derive(Debug, Args)]
struct ValidateArgs {
    #[arg(long)]
    spotify_export: Option<PathBuf>,

    #[arg(long)]
    test_mode: bool,

    #[arg(long)]
    production: bool,

    #[arg(long)]
    reset: bool,

    #[arg(long)]
    source_library: Option<PathBuf>,

    #[arg(long)]
    test_library: Option<PathBuf>,

    #[arg(long)]
    test_staging: Option<PathBuf>,

    #[arg(long)]
    test_quarantine: Option<PathBuf>,

    #[arg(long)]
    test_db: Option<PathBuf>,

    #[arg(long)]
    copy_limit: Option<usize>,

    #[arg(long)]
    no_cleanup: bool,

    #[arg(long)]
    strict_acquisition: bool,

    #[arg(long)]
    json_output: Option<PathBuf>,

    /// Preview what Custodian would do without moving any files.
    #[arg(long)]
    dry_run: bool,

    /// Only run Librarian scan + Custodian organization (skip download/gatekeeper).
    #[arg(long)]
    organize_only: bool,

    /// Resume from download phase using existing delta_queue (skip import/scan/reconcile/delta).
    #[arg(long)]
    download_only: bool,
}

#[derive(Debug, Args)]
struct LineageArgs {
    #[arg(long)]
    db: PathBuf,

    #[arg()]
    file_path: String,
}

#[derive(Debug, Args)]
struct OperationArgs {
    #[arg(long)]
    db: PathBuf,

    #[arg(long)]
    id: String,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate(args) => run_validate(args).await,
        Commands::Lineage(args) => run_lineage(args).await,
        Commands::Operation(args) => run_operation(args).await,
    }
}

async fn run_validate(args: ValidateArgs) -> Result<(), String> {
    let mut test_library = TestLibraryConfig::default();

    if let Some(value) = args.source_library {
        test_library.source_library = Some(value);
    }
    if let Some(value) = args.test_library {
        test_library.test_library = value;
    }
    if let Some(value) = args.test_staging {
        test_library.test_staging = value;
    }
    if let Some(value) = args.test_quarantine {
        test_library.test_quarantine = value;
    }
    if let Some(value) = args.test_db {
        test_library.test_db = value;
    }
    if let Some(value) = args.copy_limit {
        test_library.copy_limit = value;
    }

    let mut validation = ValidationConfig {
        test_library,
        cleanup_after_run: !args.no_cleanup,
        ..ValidationConfig::default()
    };

    if args.strict_acquisition {
        // Strict acquisition disables local-cache hits and forces external source attempts.
        validation.enabled_sources = vec![
            "http".to_string(),
            "youtube".to_string(),
            "bandcamp".to_string(),
            "spotify".to_string(),
        ];
    }

    if args.production {
        // Production mode: operate directly on the real library — no sandbox copy.
        let library_base = std::env::var("LIBRARY_BASE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("A:\\Music"));
        let staging_base = std::env::var("STAGING_FOLDER")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("A:\\Staging"));
        validation.cleanup_after_run = false;
        validation.test_library.source_library = None; // don't copy — scan in place
        validation.test_library.test_library = library_base;
        validation.test_library.test_staging = staging_base.clone();
        validation.test_library.test_quarantine = staging_base.join("quarantine");
        validation.test_library.test_db = PathBuf::from("cassette.db");
        validation.production = true;
    }

    if args.dry_run {
        validation.dry_run = true;
    }

    if args.organize_only {
        validation.organize_only = true;
        validation.run_director = false;
        validation.run_gatekeeper = false;
    }

    if args.download_only {
        validation.download_only = true;
    }

    if args.reset {
        reset_validation(&validation)
            .await
            .map_err(|e| e.to_string())?;
        println!("Validation sandbox reset complete.");
        return Ok(());
    }

    let spotify_export = match args.spotify_export {
        Some(path) => path,
        None if args.organize_only || args.download_only => {
            // These modes don't need a Spotify export — create an empty temp file.
            let empty = PathBuf::from("cassette_empty_export.json");
            if !empty.exists() {
                tokio::fs::write(&empty, "[]")
                    .await
                    .map_err(|e| e.to_string())?;
            }
            empty
        }
        None => {
            return Err(
                "--spotify-export is required unless --reset or --organize-only is used"
                    .to_string(),
            );
        }
    };

    let report = run_full_validation(&spotify_export, &validation)
        .await
        .map_err(|e| e.to_string())?;

    println!("{}", report.summary());

    if let Some(path) = args.json_output {
        let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        tokio::fs::write(&path, json)
            .await
            .map_err(|e| e.to_string())?;
        println!("Wrote validation JSON report to {}", path.display());
    }

    Ok(())
}

async fn run_lineage(args: LineageArgs) -> Result<(), String> {
    let manager = connect_manager(&args.db).await?;
    let events = get_file_lineage(&manager, &args.file_path)
        .await
        .map_err(|e| e.to_string())?;

    if events.is_empty() {
        println!("No lineage events found for {}", args.file_path);
        return Ok(());
    }

    for event in events {
        println!(
            "{} | {} | {} | {} | {}",
            event
                .timestamp
                .unwrap_or_else(|| "<no timestamp>".to_string()),
            event.operation_id,
            event.module,
            event.phase,
            event.event_type,
        );
        if let Some(data) = event.event_data {
            println!("  {}", data);
        }
    }

    Ok(())
}

async fn run_operation(args: OperationArgs) -> Result<(), String> {
    let manager = connect_manager(&args.db).await?;
    let summary = get_operation_summary(&manager, &args.id)
        .await
        .map_err(|e| e.to_string())?;

    println!(
        "operation={} module={} phase={} status={} started={} ended={}",
        summary.operation.operation_id,
        summary.operation.module,
        summary.operation.phase,
        summary.operation.status,
        summary.operation.started_at,
        summary
            .operation
            .ended_at
            .unwrap_or_else(|| "<in_progress>".to_string())
    );

    for event in summary.events {
        println!(
            "  event#{} {} {}",
            event.event_id,
            event
                .timestamp
                .unwrap_or_else(|| "<no timestamp>".to_string()),
            event.event_type
        );
    }

    Ok(())
}

async fn connect_manager(db_path: &PathBuf) -> Result<LibraryManager, String> {
    let url = sqlite_url_for_path(db_path);
    LibraryManager::connect(&url, ManagerConfig::default())
        .await
        .map_err(|e| e.to_string())
}
