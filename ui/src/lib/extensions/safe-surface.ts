export type ExtensionCapability =
  | 'visualizer.render'
  | 'visualizer.preset.read'
  | 'ui.theme.overlay'
  | 'metadata.context.read'
  | 'provider.search.adapter'
  | 'telemetry.health.emit';

export type ExtensionCategory = 'visual-pack' | 'enricher' | 'provider-adapter';

export interface SafeExtensionManifest {
  id: string;
  label: string;
  category: ExtensionCategory;
  description: string;
  capabilities: ExtensionCapability[];
  deterministicCoreAccess: false;
}

export interface ExtensionHealthReport {
  status: 'healthy' | 'degraded' | 'disabled';
  message: string;
  checkedAt: string;
  successCount: number;
  failureCount: number;
}

export const SAFE_EXTENSIONS: readonly SafeExtensionManifest[] = [
  {
    id: 'visual_pack_butterchurn',
    label: 'Butterchurn Visual Pack',
    category: 'visual-pack',
    description: 'MilkDrop-style shader preset renderer for optional visualizer lane.',
    capabilities: ['visualizer.render', 'visualizer.preset.read', 'ui.theme.overlay', 'telemetry.health.emit'],
    deterministicCoreAccess: false,
  },
  {
    id: 'enricher_lastfm_context',
    label: 'Last.fm Context Enricher',
    category: 'enricher',
    description: 'Adds artist tags/listener context for now-playing explainability surfaces.',
    capabilities: ['metadata.context.read', 'telemetry.health.emit'],
    deterministicCoreAccess: false,
  },
  {
    id: 'provider_adapter_local_archive',
    label: 'Local Archive Provider Adapter',
    category: 'provider-adapter',
    description: 'Read-only provider adapter for local-file search/acquire fallback paths.',
    capabilities: ['provider.search.adapter', 'telemetry.health.emit'],
    deterministicCoreAccess: false,
  },
] as const;

export function extensionEnabledKey(id: string): string {
  return `ui_extension_enabled_${id}`;
}

export function extensionTelemetryKey(): string {
  return 'ui_extension_health_telemetry_json';
}
