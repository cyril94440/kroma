import type { Status } from '@kromatv/client/live';
import { liveStage } from '@kromatv/core';
import { useT } from '@kromatv/ui';
import { Box, Spinner, Text } from '@kromatv/ui/kit';

/** Why there is no picture: the stream is waking, the channel is changing, the
 *  channel went off air or the provider is down. Over a picture that plays it
 *  draws nothing. */
export function LiveStateCard({
  status,
  failed,
}: Readonly<{ status: Status | null; failed: boolean }>) {
  const t = useT();
  const stage = failed ? 'error' : liveStage(status);
  if (stage === 'playing') return null;
  const title = {
    standby: status?.state === 'paused' ? t('live.paused') : t('live.standby'),
    switching: t('live.switching'),
    offAir: t('live.offAir', { name: status?.offAir?.name ?? status?.channel?.name ?? '' }),
    error: failed ? t('live.lost') : t('live.error'),
  }[stage];
  const hint =
    stage === 'standby' ? t('live.standbyHint') : stage === 'error' ? t('live.errorHint') : null;
  return (
    <Box fill center gap={20} px={64} z={8} pointerEvents="none">
      {stage === 'error' || stage === 'offAir' ? null : <Spinner size={56} />}
      <Text variant="headingTv" textAlign="center" color="white">
        {title}
      </Text>
      {hint ? (
        <Text variant="bodyTv" textAlign="center" color="text/72">
          {hint}
        </Text>
      ) : null}
    </Box>
  );
}
