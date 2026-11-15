import type { MatchPayload } from '@kromatv/client/sport';
import { calculateMatch, type FeedItem, type MatchView } from '@kromatv/core';
import { useT } from '@kromatv/ui';
import { Box, Drawer, Text } from '@kromatv/ui/kit';
import { useMemo } from 'react';

function clockLabel(view: MatchView, t: ReturnType<typeof useT>): string {
  if (view.clock.finished) return t('live.matchFinished');
  if (!view.clock.started) return t('live.matchPre');
  return view.clock.label;
}

function feedLabel(item: FeedItem, t: ReturnType<typeof useT>): string {
  if (item.kind === 'but') return item.ownGoal ? t('live.feed.ownGoal') : t('live.feed.but');
  if (item.kind === 'jaune') return t('live.feed.jaune');
  if (item.kind === 'rouge') return t('live.feed.rouge');
  return t('live.feed.remplacement');
}

function FeedRow({ item }: Readonly<{ item: FeedItem }>) {
  const t = useT();
  const who = item.player ?? item.cameOn ?? '';
  return (
    <Box row gap={14}>
      <Text variant="labelTv" color="accentText">
        {item.minute}
      </Text>
      <Text variant="bodyTv" flex>
        {`${feedLabel(item, t)} ${who}`.trim()}
      </Text>
    </Box>
  );
}

/** The match panel, computed from what this viewer's own picture has already
 *  shown: a goal appears when the frame with the goal does, never before. */
export function MatchDrawer({
  open,
  match,
  frameTimeMs,
  now,
  onClose,
}: Readonly<{
  open: boolean;
  match: MatchPayload | null;
  frameTimeMs: number | null;
  now: number;
  onClose: () => void;
}>) {
  const t = useT();
  const view = useMemo(
    () => (match ? calculateMatch(match, { pdt: frameTimeMs, now, postSeenAt: null }) : null),
    [match, frameTimeMs, now],
  );
  return (
    <Drawer.Root open={open} onClose={onClose} title={t('live.match')} width="lg" floating>
      <Drawer.Panel>
        {match && view ? (
          <Box gap={18}>
            <Text variant="labelTv" color="textMuted">
              {match.competition}
            </Text>
            <Box row justify="space-between" align="center">
              <Text variant="headingTv">{match.equipes.dom.nom}</Text>
              <Text variant="bannerTv">{`${view.score.dom} - ${view.score.ext}`}</Text>
              <Text variant="headingTv">{match.equipes.ext.nom}</Text>
            </Box>
            <Text variant="labelTv" color="accentText" textAlign="center">
              {clockLabel(view, t)}
            </Text>
            {view.stats.map((stat) => (
              <Box key={stat.key} row justify="space-between">
                <Text variant="bodyTv">{stat.dom}</Text>
                <Text variant="labelTv" color="textMuted">
                  {t(`live.stat.${stat.key}`)}
                </Text>
                <Text variant="bodyTv">{stat.ext}</Text>
              </Box>
            ))}
            <Text variant="overlineTv" color="accentText">
              {t('live.matchFeed')}
            </Text>
            {view.feed.slice(0, 6).map((item) => (
              <FeedRow key={item.id} item={item} />
            ))}
          </Box>
        ) : (
          <Text variant="bodyTv" color="textDim">
            {t('live.matchNone')}
          </Text>
        )}
      </Drawer.Panel>
    </Drawer.Root>
  );
}
