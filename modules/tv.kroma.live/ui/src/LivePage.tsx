import { type KromaHost, ModuleFailed, ModuleLoading, useT } from '@kromatv/module-sdk';
import { Box, Callout, PageHeader, Row, Text } from '@kromatv/ui/kit';
import { ChannelPanel } from './channel-panel';
import { LivePlayer } from './player';
import { NowNext } from './now-next';
import { StatusBadges } from './status-badges';
import { useLive, useNow } from './use-live';
import { ViewersPanel } from './viewers-panel';

export default function LivePage({ host }: Readonly<{ host: KromaHost }>) {
  const t = useT();
  const { api, status, failed, refresh } = useLive(host, true);
  const clock = useNow(5000);
  if (!status) return failed ? <ModuleFailed /> : <ModuleLoading panels={2} />;
  const url = status.playlist ? api.fileUrl(status.playlist) : null;

  return (
    <>
      <PageHeader.Root>
        <PageHeader.Title>{status.channel?.name ?? t('live.title')}</PageHeader.Title>
        <PageHeader.Subtitle>{t('live.subtitle')}</PageHeader.Subtitle>
      </PageHeader.Root>
      <Row wrap gap={24} mt={24} align="flex-start">
        <Box flex minW={320} style={{ flexBasis: 640 }}>
          {url ? (
            <LivePlayer url={url} title={status.channel?.name ?? t('live.title')} onLost={refresh} />
          ) : (
            <Callout.Root>
              <Text>{t(status.state === 'paused' ? 'live.standby.resuming' : 'live.player.idle')}</Text>
            </Callout.Root>
          )}
          <Box mt={12}>
            <StatusBadges status={status} />
          </Box>
          {status.lastError ? <Text variant="meta" color="textDim">{t('live.error.last')}</Text> : null}
          {status.offAir ? <Text variant="meta">{t('live.offAir.notice', { name: status.offAir.name })}</Text> : null}
          <Box mt={12}>
            <NowNext now={status.now} next={status.next} clock={clock} />
          </Box>
        </Box>
        <Box flex minW={280} style={{ flexBasis: 320 }}>
          <ChannelPanel api={api} current={status.channel?.id ?? null} onSwitched={refresh} />
          <Box mt={20}>
            <ViewersPanel viewers={status.viewers} />
          </Box>
        </Box>
      </Row>
    </>
  );
}
