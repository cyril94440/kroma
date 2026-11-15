import type { SportEvent, SportView } from '@kromatv/client/sport';
import { sportRows } from '@kromatv/core';
import type { LiveClient } from '@kromatv/core/react';
import { useT } from '@kromatv/ui';
import { Box, Drawer, SegmentGroup } from '@kromatv/ui/kit';
import { useMemo, useState } from 'react';
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
  const rows = useMemo(() => (view ? sportRows(view) : { live: [], upcoming: [] }), [view]);
  return (
    <Drawer.Root open={open} onClose={onClose} title={t('live.sport')} width="xl" floating>
      <Drawer.Panel>
        <Box gap={18}>
          <SegmentGroup.Root<Tab> value={tab} onValueChange={setTab} label={t('live.sport')} stretch>
            <SegmentGroup.Item value="rows">
              <SegmentGroup.Label>{t('live.sport.live')}</SegmentGroup.Label>
            </SegmentGroup.Item>
            <SegmentGroup.Item value="standings">
              <SegmentGroup.Label>{t('live.standings')}</SegmentGroup.Label>
            </SegmentGroup.Item>
          </SegmentGroup.Root>
          {tab === 'rows' ? <SportRows rows={rows} onWatch={onWatch} /> : null}
          {tab === 'standings' && open ? <StandingsTab client={client} /> : null}
        </Box>
      </Drawer.Panel>
    </Drawer.Root>
  );
}
