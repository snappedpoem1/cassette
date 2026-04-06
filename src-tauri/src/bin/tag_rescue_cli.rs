use cassette_core::db::Db;
use cassette_core::library::track_number_repair::{
    build_track_repair_plan, RepairRow, UnresolvedRow,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
struct RepairReport {
    inspected: usize,
    updated: usize,
    unresolved: usize,
    repaired_rows: Vec<RepairRow>,
    unresolved_rows: Vec<UnresolvedRow>,
    repair_sources: BTreeMap<String, usize>,
}

fn app_db_path() -> Result<PathBuf, String> {
    let app_data = std::env::var("APPDATA").map_err(|e| e.to_string())?;
    Ok(PathBuf::from(app_data)
        .join("dev.cassette.app")
        .join("cassette.db"))
}

fn parse_report_path(args: &[String]) -> Option<PathBuf> {
    args.windows(2)
        .find(|window| window[0] == "--report")
        .map(|window| PathBuf::from(&window[1]))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let dry_run = args.iter().any(|arg| arg == "--dry-run");
    let report_path = parse_report_path(&args);

    let db_path = app_db_path()?;
    println!("DB: {}", db_path.display());

    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    let tracks = db.get_all_tracks_unfiltered().map_err(|e| e.to_string())?;
    let plan = build_track_repair_plan(&tracks);

    if !dry_run {
        for repair in &plan.repaired {
            db.update_track_embedded_metadata(
                repair.track_id,
                None,
                None,
                None,
                repair.new_track_number,
                repair.new_disc_number,
                None,
            )
            .map_err(|e| e.to_string())?;
        }
    }

    let mut repair_sources = BTreeMap::new();
    for repair in &plan.repaired {
        *repair_sources
            .entry(repair.repair_source.as_str().to_string())
            .or_insert(0) += 1;
    }

    let report = RepairReport {
        inspected: tracks.len(),
        updated: plan.repaired.len(),
        unresolved: plan.unresolved.len(),
        repaired_rows: plan.repaired.clone(),
        unresolved_rows: plan.unresolved.clone(),
        repair_sources,
    };

    if let Some(path) = report_path {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
        println!("Wrote repair report: {}", path.display());
    }

    println!(
        "Tag rescue complete: inspected={} updated={} unresolved={} dry_run={}",
        report.inspected, report.updated, report.unresolved, dry_run
    );

    for (source, count) in &report.repair_sources {
        println!("  {source}: {count}");
    }

    if !report.unresolved_rows.is_empty() {
        println!("Unresolved rows: {}", report.unresolved_rows.len());
        for row in report.unresolved_rows.iter().take(20) {
            println!("  {} [{}]", row.path, row.reason);
        }
        if report.unresolved_rows.len() > 20 {
            println!("  ... and {} more", report.unresolved_rows.len() - 20);
        }
    }

    Ok(())
}
