// Barrel exports for $lib
export { api } from './api/tauri';
export type {
  Track,
  Album,
  Artist,
  LibraryRoot,
  QueueItem,
  PlaybackState,
  NowPlayingContext,
  ScanProgress,
  Playlist,
  PlaylistItem,
  DownloadJob,
  DownloadConfig,
  ProviderStatus,
  DownloadArtistResult,
  DownloadAlbumResult,
  DownloadMetadataSearchResult,
  DownloadArtistDiscography,
  AcquisitionQueueReport,
  SpotifyAlbumSummary,
  SpotifyImportResult,
} from './api/tauri';
export { formatDuration, formatFileSize, formatDate, formatAudioSpec, coverSrc, clamp, debounce, initials } from './utils';
