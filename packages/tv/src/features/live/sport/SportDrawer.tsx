import type { SportEvent, SportView } from '@kromatv/client/sport';
import { sportRows } from '@kromatv/core';
import type { LiveClient } from '@kromatv/core/react';
import { useT } from '@kromatv/ui';
import { Box, Drawer } from '@kromatv/ui/kit';
import { useMemo, useState } from 'react';
import { TabStrip } from '#tv/features/live/TabStrip';
import { SportRows } from '#tv/features/live/sport/SportRows';
import { StandingsTab } from '#tv/features/live/sport/StandingsTab';

type Tab = 'rows' | 'standings';

/** What is on air and coming up in sport, with the live standings; pressing a
 *  match puts its channel on the way the module decides. */
export function SportDrawer({
  open,
  view,
  client,
  onClose,
  onWatch,
}: Readonly<{
  open: boolean;
  view: SportView | null;
  client: Pick<LiveClient, 'sport'>;
  onClose: () => void;
  onWatch: (event: SportEvent) => void;
}>) {
  const t = useT();
  const [tab, setTab] = useState<Tab>('rows');
  const tabs = useMemo(
    () => [
      { id: 'rows' as const, label: t('live.sport.live') },
      { id: 'standings' as const, label: t('live.standings') },
    ],
    [t],
  );
  const rows = useMemo(() => (view ? sportRows(view) : { live: [], upcoming: [] }), [view]);
  return (
    <Drawer.Root open={open} onClose={onClose} title={t('live.sport')} width="xl" floating>
      <Drawer.Panel>
        <Box gap={18}>
          <TabStrip tabs={tabs} value={tab} onChange={setTab} />
          {tab === 'rows' ? <SportRows rows={rows} onWatch={onWatch} /> : null}
          {tab === 'standings' && open ? <StandingsTab client={client} /> : null}
        </Box>
      </Drawer.Panel>
    </Drawer.Root>
  );
}
