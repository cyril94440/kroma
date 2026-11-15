import type { Fact, MatchPayload } from '@kromatv/client/sport';

export interface RevealContext {
  pdt: number | null;
  now: number;
  delay: number;
}

const CAP_BY_PERIOD: Record<number, number> = { 1: 45, 2: 90, 3: 105, 4: 120 };

export function isRevealed(
  fact: { t_image: number | null; t_vu: number },
  context: RevealContext,
): boolean {
  if (fact.t_image !== null && context.pdt !== null) return context.pdt >= fact.t_image;
  return context.now >= fact.t_vu + context.delay * 1000;
}

function minuteLabel(seconds: number, period: number): string {
  if (period === 5) return 'TAB';
  const cap = CAP_BY_PERIOD[period] ?? 90;
  const minute = Math.floor(seconds / 60) + 1;
  return minute > cap ? `${cap}+'` : `${minute}'`;
}

export interface Clock {
  label: string;
  seconds: number | null;
  started: boolean;
  finished: boolean;
}

export interface ClockContext {
  pdt: number | null;
  now: number;
  postSeenAt: number | null;
}

export function displayedClock(
  match: MatchPayload,
  context: ClockContext,
  revealed: Fact[],
): Clock {
  const delayMs = match.delai_s * 1000;
  const allSeen = revealed.length === match.evenements.length;
  if (
    match.statut === 'post' &&
    allSeen &&
    context.postSeenAt !== null &&
    context.now >= context.postSeenAt + delayMs
  ) {
    return { label: 'finished', seconds: null, started: true, finished: true };
  }
  const reference = context.pdt ?? context.now - delayMs;
  let latest: MatchPayload['periodes'][number] | null = null;
  for (const period of match.periodes) {
    if (
      period.t_image !== null &&
      reference >= period.t_image &&
      (!latest || period.t_image >= (latest.t_image ?? 0))
    ) {
      latest = period;
    }
  }
  if (latest?.t_image != null) {
    const seconds = Math.max(0, latest.s0 + (reference - latest.t_image) / 1000);
    return { label: minuteLabel(seconds, latest.periode), seconds, started: true, finished: false };
  }
  const last = revealed.at(-1);
  if (last) {
    return {
      label: last.minute || minuteLabel(last.s, 1),
      seconds: last.s,
      started: true,
      finished: false,
    };
  }
  return { label: 'pre', seconds: null, started: false, finished: false };
}
