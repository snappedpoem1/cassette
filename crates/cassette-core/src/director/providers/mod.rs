pub mod crypto;
pub mod deezer;
pub mod local_archive;
pub mod qobuz;
pub mod slskd;
pub mod usenet;
pub mod ytdlp;

pub use deezer::DeezerProvider;
pub use local_archive::LocalArchiveProvider;
pub use qobuz::QobuzProvider;
pub use slskd::SlskdProvider;
pub use usenet::UsenetProvider;
pub use ytdlp::YtDlpProvider;
