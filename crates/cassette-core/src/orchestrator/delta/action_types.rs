use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeltaActionType {
    MissingDownload,
    UpgradeQuality,
    ManualReview,
    DuplicateReview,
    NoAction,
}

impl DeltaActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MissingDownload => "missing_download",
            Self::UpgradeQuality => "upgrade_quality",
            Self::ManualReview => "manual_review",
            Self::DuplicateReview => "duplicate_review",
            Self::NoAction => "no_action",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "missing_download" => Some(Self::MissingDownload),
            "upgrade_quality" => Some(Self::UpgradeQuality),
            "manual_review" => Some(Self::ManualReview),
            "duplicate_review" => Some(Self::DuplicateReview),
            "no_action" => Some(Self::NoAction),
            _ => None,
        }
    }
}

impl std::fmt::Display for DeltaActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
