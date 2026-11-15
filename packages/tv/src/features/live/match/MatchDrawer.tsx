import type { Clip, MatchPayload, PronoPayload } from '@kromatv/client/sport';
import { calculateMatch, clockWords, visibleClips } from '@kromatv/core';
import type { MessageKey } from '@kromatv/i18n';
import { useT } from '@kromatv/ui';
import { Box, Drawer, SegmentGroup, Text } from '@kromatv/ui/kit';
import { useMemo, useState } from 'react';
import { ClipsTab } from '#tv/features/live/match/ClipsTab';
import { PitchTab } from '#tv/features/live/match/PitchTab';
import { PronoTab } from '#tv/features/live/match/PronoTab';
import { StatsTab } from '#tv/features/live/match/StatsTab';
import { SummaryTab } from '#tv/features/live/match/SummaryTab';

const TABS = ['summary', 'lineups', 'stats', 'clips', 'prono'] as const;
type Tab = (typeof TABS)[number];

const TAB_LABEL: Record<Tab, MessageKey> = {
  summary: 'live.tab.summary',
  lineups: 'live.tab.lineups',
  stats: 'live.tab.stats',
  clips: 'live.clips',
  prono: 'live.prono',
};

export interface MatchPanelData {
  match: MatchPayload | null;
  prono: PronoPayload | null;
  clips: readonly Clip[];
  me: string;
  postSeenAt: number | null;
}

/** The match panel, computed from what this viewer's own picture has already
 *  shown: a goal appears when the frame with the goal does, never before. */
export function MatchDrawer({
  open,
  data,
  frameTimeMs,
  now,
  onClose,
  onVote,
  onPlayClip,
}: Readonly<{
  open: boolean;
  data: MatchPanelData;
  frameTimeMs: () => number | null;
  now: number;
  onClose: () => void;
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
    <Drawer.Root open={open} onClose={onClose} title={t('live.match')} width="xl" floating>
      <Drawer.Panel>
        <Box gap={18}>
          {match && view ? (
            <>
              <Text variant="labelTv" color="textMuted">
                {match.competition}
              </Text>
              <Box row justify="space-between" align="center">
                <Text variant="headingTv">{match.equipes.dom.nom}</Text>
                <Text variant="bannerTv">{`${view.score.dom} - ${view.score.ext}`}</Text>
                <Text variant="headingTv">{match.equipes.ext.nom}</Text>
              </Box>
              <Text variant="labelTv" color="accentText" textAlign="center">
                {words ? t(words) : view.clock.label}
              </Text>
            </>
          ) : null}
          <SegmentGroup.Root<Tab> value={tab} onValueChange={setTab} label={t('live.tabs')} stretch>
            {TABS.map((id) => (
              <SegmentGroup.Item key={id} value={id}>
                <SegmentGroup.Label>{t(TAB_LABEL[id])}</SegmentGroup.Label>
              </SegmentGroup.Item>
            ))}
          </SegmentGroup.Root>
          {tab === 'summary' && view ? <SummaryTab view={view} /> : null}
          {tab === 'lineups' && match && view ? (
            <PitchTab teams={match.equipes} onPitch={view.onPitch} />
          ) : null}
          {tab === 'stats' && view ? <StatsTab view={view} /> : null}
          {tab === 'clips' ? <ClipsTab clips={shownClips} onPlay={onPlayClip} /> : null}
          {tab === 'prono' ? <PronoTab prono={prono} me={me} onVote={onVote} /> : null}
          {!match && (tab === 'summary' || tab === 'lineups' || tab === 'stats') ? (
            <Text variant="bodyTv" color="textDim">
              {t('live.matchNone')}
            </Text>
          ) : null}
        </Box>
      </Drawer.Panel>
    </Drawer.Root>
  );
}
