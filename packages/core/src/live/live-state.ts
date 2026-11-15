import { Health, LiveEvent, type Status } from '@kromatv/client/live';

export function applyLiveEvent(status: Status | null, raw: unknown): Status | null {
  const parsed = LiveEvent.safeParse(raw);
  if (!parsed.success) return status;
  const event = parsed.data;
  switch (event.type) {
    case 'live.status': {
      const { type: _type, ...next } = event;
      return next;
    }
    case 'live.presence':
      return status ? { ...status, viewers: event.viewers } : status;
    case 'live.health': {
      const { type: _type, ...health } = event;
      return status ? { ...status, health: Health.parse(health) } : status;
    }
    default:
      return status;
  }
}

export type LiveBadge = 'backup' | 'primaryDown' | 'black' | 'frozen' | 'stutter' | 'offAir';

export function liveBadges(status: Status): readonly LiveBadge[] {
  const out: LiveBadge[] = [];
  if (status.onBackup) out.push('backup');
  if (status.primaryDown) out.push('primaryDown');
  if (status.health.black) out.push('black');
  if (status.health.frozen) out.push('frozen');
  if (status.health.stutter) out.push('stutter');
  if (status.state === 'offAir') out.push('offAir');
  return out;
}

export function slotProgress(now: number, slot: { start: number; end: number } | null): number {
  if (!slot || slot.end <= slot.start) return 0;
  return Math.min(1, Math.max(0, (now - slot.start) / (slot.end - slot.start)));
}

export type LiveStage = 'standby' | 'switching' | 'offAir' | 'error' | 'playing';

/** What the screen should show for a status: the picture, or the reason there
 *  is none. A paused module and a running one with no playlist yet are both
 *  standby from the viewer's side. */
export function liveStage(status: Status | null): LiveStage {
  if (!status) return 'standby';
  if (status.state === 'switching') return 'switching';
  if (status.state === 'offAir') return 'offAir';
  if (status.state === 'error') return 'error';
  if (status.state === 'paused' || !status.playlist) return 'standby';
  return 'playing';
}

export function playlistKey(status: Status | null): string | null {
  return status?.state === 'running' ? status.playlist : null;
}
