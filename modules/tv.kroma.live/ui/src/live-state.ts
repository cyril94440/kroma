import { Health, LiveEvent, type Status } from './schemas';

export function applyEvent(status: Status | null, raw: unknown): Status | null {
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

export function shouldReloadPlayer(before: string | null, after: string | null): boolean {
  return before !== after && after !== null;
}

export function badges(status: Status): readonly string[] {
  const out: string[] = [];
  if (status.onBackup) out.push('backup');
  if (status.primaryDown) out.push('primaryDown');
  if (status.health.black) out.push('black');
  if (status.health.frozen) out.push('frozen');
  if (status.health.stutter) out.push('stutter');
  if (status.state === 'offAir') out.push('offAir');
  return out;
}

export function progressOf(now: number, slot: { start: number; end: number } | null): number {
  if (!slot || slot.end <= slot.start) return 0;
  return Math.min(1, Math.max(0, (now - slot.start) / (slot.end - slot.start)));
}
