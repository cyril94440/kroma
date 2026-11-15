import { useT } from '@kromatv/module-sdk';
import { Box, Row, SegmentGroup, Text } from '@kromatv/ui/kit';
import { useState } from 'react';
import { MatchSummary } from './match-summary';
import type { MatchView } from './match-calc';
import { Pitch } from './pitch';
import type { MatchPayload } from './schemas';
import { StatsView } from './stats-view';

const TABS = ['summary', 'lineups', 'stats'] as const;
type Tab = (typeof TABS)[number];

export function clockText(view: MatchView, t: ReturnType<typeof useT>): string {
  if (view.clock.finished) return t('sport.match.finished');
  return view.clock.started ? view.clock.label : t('sport.match.pre');
}

interface MatchPanelProps {
  match: MatchPayload;
  view: MatchView;
}

export function MatchPanel({ match, view }: Readonly<MatchPanelProps>) {
  const t = useT();
  const [tab, setTab] = useState<Tab>('summary');
  return (
    <Box data-testid="sport-match">
      <Row between align="center" mb={8}>
        <Text variant="title">{match.equipes.dom.abbr || match.equipes.dom.nom}</Text>
        <Text
          variant="title"
          data-testid="sport-score"
        >{`${view.score.dom} – ${view.score.ext}`}</Text>
        <Text variant="title">{match.equipes.ext.abbr || match.equipes.ext.nom}</Text>
      </Row>
      <Text variant="meta" color="textDim">{`${match.competition} · ${clockText(view, t)}`}</Text>
      <SegmentGroup.Root<Tab>
        value={tab}
        onValueChange={setTab}
        label={t('sport.match.views')}
        size="sm"
        stretch
      >
        {TABS.map((id) => (
          <SegmentGroup.Item key={id} value={id}>
            <SegmentGroup.Label>{t(`sport.match.tab.${id}`)}</SegmentGroup.Label>
          </SegmentGroup.Item>
        ))}
      </SegmentGroup.Root>
      <Box mt={12}>
        {tab === 'summary' ? <MatchSummary match={match} view={view} /> : null}
        {tab === 'lineups' ? <Pitch teams={match.equipes} onPitch={view.onPitch} /> : null}
        {tab === 'stats' ? <StatsView stats={view.stats} momentum={view.momentum} /> : null}
      </Box>
    </Box>
  );
}
