use crate::librarian::models::DeltaActionType;

pub fn reason_for_action(action: DeltaActionType, reason: &str) -> String {
    let prefix = match action {
        DeltaActionType::MissingDownload => "missing",
        DeltaActionType::UpgradeQuality => "upgrade",
        DeltaActionType::RelinkMetadata => "relink",
        DeltaActionType::ManualReview => "manual",
        DeltaActionType::DuplicateReview => "duplicate",
        DeltaActionType::NoAction => "ok",
    };
    format!("{prefix}: {reason}")
}
