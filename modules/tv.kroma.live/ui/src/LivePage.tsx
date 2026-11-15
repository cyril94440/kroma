import { ModuleFailed, ModuleLoading, ModuleSlot, useSlotFilled, useT } from '@kromatv/module-sdk';
import { useState } from 'react';
import { Box, Callout, PageHeader, Row, Text } from '@kromatv/ui/kit';
import { ChannelPanel } from './channel-panel';
import { LivePlayer } from './player';
import type { PlayerHandle } from './player-handle';
import { NowNext } from './now-next';
import { StatusBadges } from './status-badges';
import { useLive, useNow } from './use-live';
import { ViewersPanel } from './viewers-panel';

const HIDDEN = { display: 'none' } as const;
const ASIDE_COLUMN = { flexBasis: 320 } as const;

export default function LivePage() {
  const t = useT();
  const { api, status, failed, refresh } = useLive(true);
  const clock = useNow(5000);
  const [handle, setHandle] = useState<PlayerHandle | null>(null);
  const [playing, setPlaying] = useState(false);
  const [immersive, setImmersive] = useState(false);
  const hasAside = useSlotFilled('tv.kroma.live/player.aside');
  if (!status) return failed ? <ModuleFailed /> : <ModuleLoading panels={2} />;
  const url = status.playlist ? api.fileUrl(status.playlist) : null;
  const channel = status.channel ? { id: status.channel.id, name: status.channel.name } : null;

  return (
    <>
      <PageHeader.Root>
        <PageHeader.Title>{status.channel?.name ?? t('live.title')}</PageHeader.Title>
        <PageHeader.Subtitle>{t('live.subtitle')}</PageHeader.Subtitle>
      </PageHeader.Root>
      <Row wrap gap={24} mt={24} align="flex-start">
        <Box flex minW={320} style={{ flexBasis: 640 }}>
          {url ? (
            <LivePlayer
              url={url}
              title={status.channel?.name ?? t('live.title')}
              onLost={refresh}
              channel={channel}
              onHandle={setHandle}
              onPlaying={setPlaying}
              onImmersive={setImmersive}
            />
          ) : (
            <Callout.Root>
              <Text>
                {t(status.state === 'paused' ? 'live.standby.resuming' : 'live.player.idle')}
              </Text>
            </Callout.Root>
          )}
          <Box mt={12}>
            <StatusBadges status={status} />
          </Box>
          {status.lastError ? (
            <Text variant="meta" color="textDim">
              {t('live.error.last')}
            </Text>
          ) : null}
          {status.offAir ? (
            <Text variant="meta">{t('live.offAir.notice', { name: status.offAir.name })}</Text>
          ) : null}
          <Box mt={12}>
            <NowNext now={status.now} next={status.next} clock={clock} />
          </Box>
        </Box>
        <Box flex minW={280} style={immersive ? HIDDEN : ASIDE_COLUMN}>
          {handle && hasAside ? (
            <Box mb={20}>
              <ModuleSlot
                name="tv.kroma.live/player.aside"
                player={handle}
                channel={channel}
                playing={playing}
              />
            </Box>
          ) : null}
          <ChannelPanel api={api} current={status.channel?.id ?? null} onSwitched={refresh} />
          <Box mt={20}>
            <ViewersPanel viewers={status.viewers} />
          </Box>
        </Box>
      </Row>
    </>
  );
}
