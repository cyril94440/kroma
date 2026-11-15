import type { Clip, MatchPayload, PronoPayload } from '@kromatv/client/sport';
import { calculateMatch, clockWords, visibleClips } from '@kromatv/core';
import { Box, SegmentGroup, Text } from '@kromatv/ui/kit';
import { useMemo, useState } from 'react';
import { useT } from '#mobile/lib/i18n';
import { MatchClips } from '#mobile/live/match/MatchClips';
import { MatchPitch } from '#mobile/live/match/MatchPitch';
import { MatchProno } from '#mobile/live/match/MatchProno';
import { MatchStats } from '#mobile/live/match/MatchStats';
import { MatchSummary } from '#mobile/live/match/MatchSummary';

const TABS = ['summary', 'lineups', 'stats', 'clips', 'prono'] as const;
type Tab = (typeof TABS)[number];

const TAB_LABEL = {
  summary: 'live.tab.summary',
  lineups: 'live.tab.lineups',
  stats: 'live.tab.stats',
  clips: 'live.clips',
  prono: 'live.prono',
} as const satisfies Record<Tab, string>;

export interface MatchPanelData {
  match: MatchPayload | null;
  prono: PronoPayload | null;
  clips: readonly Clip[];
  me: string;
  postSeenAt: number | null;
}

/** The match this viewer's own picture has reached: the score, the clock, the
 *  line-ups, the counters and the key moments, each shown only once the frame
 *  carrying it has played; then the goal clips and the prediction. */
export function LiveMatch({
  data,
  frameTimeMs,
  now,
  onVote,
  onPlayClip,
}: Readonly<{
  data: MatchPanelData;
  frameTimeMs: () => number | null;
  now: number;
  onVote: (id: string, score: string) => void;
  onPlayClip: (clip: Clip) => void;
}>) {
  const t = useT();
  const [tab, setTab] = useState<Tab>('summary');
  const { match, prono, clips, me, postSeenAt } = data;
  const pdt = frameTimeMs();
  const view = useMemo(
    () => (match ? calculateMatch(match, { pdt, now, postSeenAt }) : null),
    [match, pdt, now, postSeenAt],
  );
  const shownClips = useMemo(
    () => visibleClips(clips, pdt, now, match?.id ?? null),
    [clips, pdt, now, match],
  );
  const words = view ? clockWords(view.clock) : null;
  return (
    <Box gap={12}>
      {match && view ? (
        <>
          <Text variant="meta" color="textMuted">
            {match.competition}
          </Text>
          <Box row justify="space-between" align="center">
            <Text variant="label">{match.equipes.dom.nom}</Text>
            <Text variant="title">{`${view.score.dom} - ${view.score.ext}`}</Text>
            <Text variant="label">{match.equipes.ext.nom}</Text>
          </Box>
          <Text variant="meta" color="accentText" textAlign="center">
            {words ? t(words) : view.clock.label}
          </Text>
        </>
      ) : null}
      <SegmentGroup.Root<Tab>
        value={tab}
        onValueChange={setTab}
        label={t('live.tabs')}
        size="sm"
        stretch
      >
        {TABS.map((id) => (
          <SegmentGroup.Item key={id} value={id}>
            <SegmentGroup.Label>{t(TAB_LABEL[id])}</SegmentGroup.Label>
          </SegmentGroup.Item>
        ))}
      </SegmentGroup.Root>
      {tab === 'summary' && view ? <MatchSummary view={view} /> : null}
      {tab === 'lineups' && match && view ? (
        <MatchPitch teams={match.equipes} onPitch={view.onPitch} />
      ) : null}
      {tab === 'stats' && view ? <MatchStats view={view} /> : null}
      {tab === 'clips' ? <MatchClips clips={shownClips} onPlay={onPlayClip} /> : null}
      {tab === 'prono' ? <MatchProno prono={prono} me={me} onVote={onVote} /> : null}
      {!match && (tab === 'summary' || tab === 'lineups' || tab === 'stats') ? (
        <Text variant="body" color="textDim">
          {t('live.matchNone')}
        </Text>
      ) : null}
    </Box>
  );
}
