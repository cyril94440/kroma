import type { MatchPayload } from '@kromatv/client/sport';
import { calculateMatch } from '@kromatv/core';
import { Box, Text } from '@kromatv/ui/kit';
import { useMemo } from 'react';
import { useT } from '#mobile/lib/i18n';

/** The match this viewer's own picture has reached: the score, the clock and
 *  the key moments, each shown only once the frame carrying it has played. */
export function LiveMatch({
  match,
  frameTimeMs,
  now,
}: Readonly<{ match: MatchPayload | null; frameTimeMs: number | null; now: number }>) {
  const t = useT();
  const view = useMemo(
    () => (match ? calculateMatch(match, { pdt: frameTimeMs, now, postSeenAt: null }) : null),
    [match, frameTimeMs, now],
  );
  if (!match || !view) {
    return (
      <Text variant="body" color="textDim">
        {t('live.matchNone')}
      </Text>
    );
  }
  const clock = view.clock.finished
    ? t('live.matchFinished')
    : view.clock.started
      ? view.clock.label
      : t('live.matchPre');
  return (
    <Box gap={12}>
      <Text variant="meta" color="textMuted">
        {match.competition}
      </Text>
      <Box row justify="space-between" align="center">
        <Text variant="label">{match.equipes.dom.nom}</Text>
        <Text variant="title">{`${view.score.dom} - ${view.score.ext}`}</Text>
        <Text variant="label">{match.equipes.ext.nom}</Text>
      </Box>
      <Text variant="meta" color="accentText" textAlign="center">
        {clock}
      </Text>
      {view.stats.map((stat) => (
        <Box key={stat.key} row justify="space-between">
          <Text variant="body">{stat.dom}</Text>
          <Text variant="meta" color="textMuted">
            {t(`live.stat.${stat.key}`)}
          </Text>
          <Text variant="body">{stat.ext}</Text>
        </Box>
      ))}
    </Box>
  );
}
