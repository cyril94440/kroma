import { useT } from '@kromatv/module-sdk';
import { Box, Button, Callout, Chip, EmptyState, Row, Section, Text } from '@kromatv/ui/kit';
import { useState } from 'react';
import { EventRow } from './event-row';
import type { SportView } from './schemas';

const FILTERS = ['all', 'foot', 'tennis', 'f1', 'golf'] as const;
type Filter = (typeof FILTERS)[number];
const UPCOMING_MAX = 8;

interface SportListProps {
  view: SportView;
  now: number;
  ownScore: string | null;
  onOpen: (force: boolean) => void;
  onWatch: (channelId: string) => void;
  message: string | null;
}

export function SportList({
  view,
  now,
  ownScore,
  onOpen,
  onWatch,
  message,
}: Readonly<SportListProps>) {
  const t = useT();
  const [filter, setFilter] = useState<Filter>('all');
  const shown = view.evenements.filter((e) => filter === 'all' || e.sport === filter);
  const live = shown.filter((e) => e.statut === 'in');
  const upcoming = shown
    .filter((e) => e.statut === 'pre')
    .sort((a, b) => b.pop - a.pop || a.debut - b.debut)
    .slice(0, UPCOMING_MAX)
    .sort((a, b) => a.debut - b.debut);
  const scanning = view.scan.etat === 'en_cours';
  return (
    <>
      <Row wrap gap={6} mb={8}>
        {FILTERS.map((f) => (
          <Chip
            key={f}
            label={t(`sport.filter.${f}`)}
            active={filter === f}
            onPress={() => setFilter(f)}
          />
        ))}
        <Button
          label={t('sport.refresh')}
          size="sm"
          variant="glass"
          loading={scanning}
          onPress={() => onOpen(true)}
        />
      </Row>
      <Text variant="meta" color="textDim">
        {t(`sport.scan.${view.scan.etat}`, { done: view.scan.fait, total: view.scan.total })}
      </Text>
      {message ? (
        <Callout.Root tone="accent">
          <Callout.Title>{message}</Callout.Title>
        </Callout.Root>
      ) : null}
      {live.length > 0 ? (
        <Section.Root>
          <Section.Header>
            <Section.Title>{t('sport.live')}</Section.Title>
          </Section.Header>
          {live.map((e) => (
            <EventRow key={e.id} event={e} now={now} ownScore={ownScore} onWatch={onWatch} />
          ))}
        </Section.Root>
      ) : null}
      {view.ligue1.canaux.length > 0 && (filter === 'all' || filter === 'foot') ? (
        <Section.Root>
          <Section.Header>
            <Section.Title>{t('sport.multiplex')}</Section.Title>
          </Section.Header>
          {view.ligue1.canaux.map((c) => (
            <Box key={c.ref} mb={6}>
              <Row between>
                <Text variant="body">
                  {[c.num, c.equipes.join(' – ')].filter(Boolean).join('  ')}
                </Text>
                <Text variant="label">{c.a_lecran ? (ownScore ?? '') : c.score}</Text>
              </Row>
            </Box>
          ))}
        </Section.Root>
      ) : null}
      {upcoming.length > 0 ? (
        <Section.Root>
          <Section.Header>
            <Section.Title>{t('sport.upcoming')}</Section.Title>
          </Section.Header>
          {upcoming.map((e) => (
            <EventRow key={e.id} event={e} now={now} ownScore={ownScore} onWatch={onWatch} />
          ))}
        </Section.Root>
      ) : null}
      {live.length === 0 && upcoming.length === 0 ? (
        <EmptyState.Root>
          <EmptyState.Title>{t('sport.empty')}</EmptyState.Title>
        </EmptyState.Root>
      ) : null}
    </>
  );
}
