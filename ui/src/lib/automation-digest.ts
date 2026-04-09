import type {
  AcquisitionRequestListItem,
  ProviderHealthEvent,
  ProviderStatus,
  SpotifyAlbumHistory,
} from '$lib/api/tauri';

export type AutomationThreshold =
  | 'silent'
  | 'digest'
  | 'soft_attention'
  | 'explicit_intervention';

export type AutomationTone = 'steady' | 'watch' | 'action';

export interface AutomationDigestLine {
  label: string;
  detail: string;
  tone: AutomationTone;
}

export interface AutomationDigestSummary {
  threshold: AutomationThreshold;
  tone: AutomationTone;
  title: string;
  detail: string;
  counts: {
    inbox: number;
    active: number;
    review: number;
    blocked: number;
    downProviders: number;
    configuredProviders: number;
  };
  lines: AutomationDigestLine[];
}

export interface AutomationDigestInput {
  requests: AcquisitionRequestListItem[];
  missingAlbums: SpotifyAlbumHistory[];
  providerHealth: Record<string, ProviderHealthEvent>;
  providerStatuses: ProviderStatus[];
  slskdReady: boolean;
  isScanning: boolean;
  backlogRunning: boolean;
  queueCount?: number;
}

export const AUTOMATION_THRESHOLD_LABELS: Record<AutomationThreshold, string> = {
  silent: 'Silent',
  digest: 'Digest',
  soft_attention: 'Soft attention',
  explicit_intervention: 'Explicit intervention',
};

export const AUTOMATION_THRESHOLD_NOTES: Record<AutomationThreshold, string> = {
  silent: 'Nothing is asking for you. Cassette can stay in the background.',
  digest: 'Background motion is real, but it should read as recap instead of interruption.',
  soft_attention: 'A lane is drifting. Surface it gently and keep the listening room intact.',
  explicit_intervention: 'A real decision or repair is waiting. Put it in Workstation on purpose.',
};

export function buildAutomationDigest(input: AutomationDigestInput): AutomationDigestSummary {
  const active = input.requests.filter((request) =>
    ['queued', 'submitted', 'in_progress'].includes(request.status),
  ).length;
  const review = input.requests.filter((request) => request.status === 'reviewing').length;
  const blocked = input.requests.filter((request) =>
    ['failed', 'cancelled'].includes(request.status),
  ).length;
  const inbox = input.missingAlbums.length;
  const downProviders = Object.values(input.providerHealth).filter(
    (provider) => provider.status === 'Down',
  ).length;
  const configuredProviders = input.providerStatuses.filter((provider) => provider.configured).length;
  const queueCount = input.queueCount ?? 0;

  let threshold: AutomationThreshold = 'silent';
  if (review > 0 || blocked > 1 || (!input.slskdReady && inbox > 0) || downProviders > 1) {
    threshold = 'explicit_intervention';
  } else if (blocked > 0 || downProviders > 0) {
    threshold = 'soft_attention';
  } else if (active > 0 || input.backlogRunning || input.isScanning || inbox > 0 || queueCount > 0) {
    threshold = 'digest';
  }

  const tone: AutomationTone =
    threshold === 'explicit_intervention'
      ? 'action'
      : threshold === 'soft_attention'
        ? 'watch'
        : 'steady';

  const lines: AutomationDigestLine[] = [];

  if (input.isScanning) {
    lines.push({
      label: 'Library motion',
      detail: 'A scan is moving quietly through the collection.',
      tone: 'steady',
    });
  }

  if (active > 0 || input.backlogRunning || queueCount > 0) {
    const parts: string[] = [];
    if (active > 0) {
      parts.push(`${active} request${active === 1 ? '' : 's'} in flight`);
    }
    if (queueCount > 0) {
      parts.push(`${queueCount} queued for playback`);
    }
    if (input.backlogRunning) {
      parts.push('backlog draining in the background');
    }
    lines.push({
      label: 'Background motion',
      detail: parts.join(' / '),
      tone: 'steady',
    });
  }

  if (review > 0 || blocked > 0) {
    const parts: string[] = [];
    if (review > 0) {
      parts.push(`${review} waiting on approval`);
    }
    if (blocked > 0) {
      parts.push(`${blocked} stalled or cancelled`);
    }
    lines.push({
      label: threshold === 'explicit_intervention' ? 'Decision point' : 'Attention line',
      detail: parts.join(' / '),
      tone: threshold === 'explicit_intervention' ? 'action' : 'watch',
    });
  }

  if (inbox > 0) {
    lines.push({
      label: 'Inbox',
      detail: `${inbox} album${inbox === 1 ? '' : 's'} still missing from the shelves.`,
      tone: 'watch',
    });
  }

  if (!input.slskdReady || downProviders > 0) {
    const parts: string[] = [];
    if (!input.slskdReady) {
      parts.push('Soulseek runtime needs a restart or check');
    }
    if (downProviders > 0) {
      parts.push(`${downProviders} provider${downProviders === 1 ? '' : 's'} reporting down`);
    }
    lines.push({
      label: 'System health',
      detail: parts.join(' / '),
      tone: threshold === 'explicit_intervention' ? 'action' : 'watch',
    });
  }

  if (lines.length === 0) {
    lines.push({
      label: 'Quiet room',
      detail: 'No blocked work, no missing shelf pressure, and no services asking for intervention.',
      tone: 'steady',
    });
  }

  const titleMap: Record<AutomationThreshold, string> = {
    silent: 'Quiet room',
    digest: 'Background motion, no interruption',
    soft_attention: 'A gentle nudge is enough',
    explicit_intervention: 'A deliberate look is warranted',
  };

  return {
    threshold,
    tone,
    title: titleMap[threshold],
    detail: AUTOMATION_THRESHOLD_NOTES[threshold],
    counts: {
      inbox,
      active,
      review,
      blocked,
      downProviders,
      configuredProviders,
    },
    lines: lines.slice(0, 4),
  };
}
