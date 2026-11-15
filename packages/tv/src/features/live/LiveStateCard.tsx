import type { Status } from '@kromatv/client/live';
import { liveNotice } from '@kromatv/core';
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
  const notice = liveNotice(status, failed);
  if (!notice) return null;
  return (
    <Box fill center gap={20} px={64} z={8} pointerEvents="none">
      {notice.spinner ? <Spinner size={56} /> : null}
      <Text variant="headingTv" textAlign="center" color="white">
        {t(notice.title, { name: notice.channelName })}
      </Text>
      {notice.hint ? (
        <Text variant="bodyTv" textAlign="center" color="text/72">
          {t(notice.hint)}
        </Text>
      ) : null}
    </Box>
  );
}
