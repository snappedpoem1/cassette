pub mod compare;
pub mod delta;

use crate::librarian::db::LibrarianDb;
use crate::librarian::error::Result;
use crate::librarian::matchers::match_desired_track;
use crate::librarian::reconcile::compare::classify_delta;
use crate::librarian::reconcile::delta::reason_for_action;
use tracing::info;

pub async fn reconcile_desired_state(db: &LibrarianDb) -> Result<usize> {
    db.clear_reconciliation().await?;
    let desired = db.list_desired_tracks().await?;

    let mut count = 0usize;
    for item in desired {
        let outcome = match_desired_track(db, &item).await?;
        let matched_file = if let Some(track_id) = outcome.matched_track_id {
            db.list_local_files_for_track(track_id)
                .await?
                .into_iter()
                .next()
        } else {
            None
        };

        let (mut recon, mut delta) = classify_delta(&outcome, matched_file.as_ref());
        recon.desired_track_id = item.id;
        delta.desired_track_id = item.id;
        delta.reason = reason_for_action(delta.action_type, &delta.reason);

        db.insert_reconciliation_result(&recon).await?;
        db.enqueue_delta(&delta).await?;
        count += 1;
    }

    info!(processed = count, "reconciliation completed");
    Ok(count)
}
