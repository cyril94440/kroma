import { ModuleFailed, ModuleLoading, useT } from '@kromatv/module-sdk';
import { Box, PageHeader, SegmentGroup } from '@kromatv/ui/kit';
import { useEffect, useState } from 'react';
import { CalibrateForm } from './calibrate-form';
import type { MatchView } from './match-calc';
import type { SportState } from './schemas';
import type { SportApi } from './sport-api';
import { ClipsPanel } from './clips-panel';
import { MatchPanel } from './match-panel';
import { PronoPanel } from './prono-panel';
import { SportList } from './sport-list';
import { StandingsPanel } from './standings-panel';
import { useMatchView } from './use-match-view';
import { useSport } from './use-sport';

const TABS = ['live', 'match', 'prono', 'standings', 'clips'] as const;
type Tab = (typeof TABS)[number];

interface TabBodyProps {
  tab: Tab;
  state: SportState;
  api: SportApi;
  view: MatchView | null;
  now: number;
  serverNow: number;
  message: string | null;
  onWatch: (channelId: string) => void;
}

function TabBody({
  tab,
  state,
  api,
  view,
  now,
  serverNow,
  message,
  onWatch,
}: Readonly<TabBodyProps>) {
  switch (tab) {
    case 'live':
      return (
        <SportList
          view={state.view}
          now={now}
          ownScore={null}
          onOpen={(force) => void api.open(force)}
          onWatch={onWatch}
          message={message}
        />
      );
    case 'match':
      return state.match && view ? (
        <>
          <MatchPanel match={state.match} view={view} />
          <Box mt={16}>
            <CalibrateForm api={api} getPositionMs={() => null} />
          </Box>
        </>
      ) : (
        <ModuleLoading panels={1} />
      );
    case 'prono':
      return (
        <PronoPanel
          prono={state.prono}
          me={state.me.userId}
          api={api}
          season={state.prono?.saison ?? []}
        />
      );
    case 'standings':
      return <StandingsPanel api={api} />;
    case 'clips':
      return <ClipsPanel clips={state.clips} pdt={null} now={serverNow} />;
  }
}

export default function SportPage() {
  const t = useT();
  const { api, model, failed, now, serverNow, postSeenAt } = useSport();
  const [tab, setTab] = useState<Tab>('live');
  const [message, setMessage] = useState<string | null>(null);
  const state = model.state;
  const view = useMatchView(state?.match ?? null, null, serverNow, postSeenAt);

  useEffect(() => {
    api.open(false).catch(() => undefined);
  }, [api]);

  if (!state) return failed ? <ModuleFailed /> : <ModuleLoading panels={2} />;
  const watch = (channelId: string) => {
    api.watch(channelId).then(
      (outcome) => setMessage(t(`sport.watch.${outcome.outcome}`)),
      () => setMessage(t('sport.watch.unavailable')),
    );
  };
  return (
    <>
      <PageHeader.Root>
        <PageHeader.Title>{t('sport.title')}</PageHeader.Title>
        <PageHeader.Subtitle>{t('sport.subtitle')}</PageHeader.Subtitle>
      </PageHeader.Root>
      <Box mt={16}>
        <SegmentGroup.Root<Tab>
          value={tab}
          onValueChange={setTab}
          label={t('sport.tabs')}
          size="sm"
          stretch
        >
          {TABS.map((id) => (
            <SegmentGroup.Item key={id} value={id}>
              <SegmentGroup.Label>{t(`sport.tab.${id}`)}</SegmentGroup.Label>
            </SegmentGroup.Item>
          ))}
        </SegmentGroup.Root>
      </Box>
      <Box mt={16}>
        <TabBody
          tab={tab}
          state={state}
          api={api}
          view={view}
          now={now}
          serverNow={serverNow}
          message={message}
          onWatch={watch}
        />
      </Box>
    </>
  );
}
