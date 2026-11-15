import type { VoteClosed } from '@kromatv/client/party';
import { useT } from '@kromatv/ui';
import { toast } from '@kromatv/ui/kit';
import { useEffect } from 'react';

const VERDICT_KEY = {
  passed: 'live.votePassed',
  refused: 'live.voteRefused',
  impossible: 'live.voteImpossible',
  cancelled: 'live.voteCancelled',
} as const satisfies Record<VoteClosed['verdict'], string>;

/** Says how a zapping vote ended, once, when the party reports it closed. */
export function useVerdictToasts(closed: (VoteClosed & { at: number }) | null): void {
  const t = useT();
  const at = closed?.at;
  const verdict = closed?.verdict;
  useEffect(() => {
    if (at === undefined || verdict === undefined) return;
    toast({ message: t(VERDICT_KEY[verdict]) });
  }, [at, verdict, t]);
}
