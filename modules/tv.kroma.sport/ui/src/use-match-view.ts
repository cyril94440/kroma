import { useMemo } from 'react';
import { calculate, type MatchView } from './match-calc';
import type { MatchPayload } from './schemas';

export function useMatchView(
  match: MatchPayload | null,
  pdt: number | null,
  serverNow: number,
  postSeenAt: number | null,
): MatchView | null {
  return useMemo(
    () => (match ? calculate(match, { pdt, now: serverNow, postSeenAt }) : null),
    [match, pdt, serverNow, postSeenAt],
  );
}
