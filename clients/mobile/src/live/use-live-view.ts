import type { LiveRoom } from '@kromatv/core/react';
import { useMemo } from 'react';
import { useT } from '#mobile/lib/i18n';
import type { MatchPanelData } from '#mobile/live/match/LiveMatch';

/** What the live screen reads off the room: the match panel's data, whether
 *  there is anything to open it for, and the agent's action line. */
export function useLiveView(room: LiveRoom) {
  const t = useT();
  const { party, sport } = room;
  const sportState = sport.state;
  const me = party.state?.me.userId ?? '';
  const match = sportState?.match ?? null;
  const prono = sportState?.prono ?? null;
  const clips = sportState?.clips;
  const postSeenAt = sport.postSeen?.at ?? null;
  const matchData = useMemo<MatchPanelData>(
    () => ({ match, prono, clips: clips ?? [], me, postSeenAt }),
    [match, prono, clips, me, postSeenAt],
  );
  const hasMatch = match !== null || prono !== null || (clips?.length ?? 0) > 0;
  const label = party.state?.activity;
  const activity = label
    ? t('live.activity', { name: party.state?.config.agentName ?? '', label })
    : null;
  return { me, matchData, hasMatch, activity, sportView: sportState?.view ?? null };
}
