use crate::director::sources::SourceProvider;
use crate::librarian::models::DesiredTrack;
use std::sync::Arc;

pub async fn available_sources_for_track(
    track: &DesiredTrack,
    sources: &[Arc<dyn SourceProvider>],
) -> Vec<Arc<dyn SourceProvider>> {
    let mut available = Vec::new();
    for source in sources {
        if !source.can_handle(track) {
            continue;
        }
        if source.check_availability(track).await.unwrap_or(false) {
            available.push(Arc::clone(source));
        }
    }
    available
}
