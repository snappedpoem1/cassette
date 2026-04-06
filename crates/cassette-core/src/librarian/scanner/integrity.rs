use crate::librarian::models::IntegrityStatus;

pub fn assess_integrity(
    has_audio_stream: bool,
    has_required_metadata: bool,
    suspicious: bool,
) -> IntegrityStatus {
    if suspicious {
        return IntegrityStatus::Suspicious;
    }
    if !has_audio_stream {
        return IntegrityStatus::Unreadable;
    }
    if !has_required_metadata {
        return IntegrityStatus::PartialMetadata;
    }
    IntegrityStatus::Readable
}
