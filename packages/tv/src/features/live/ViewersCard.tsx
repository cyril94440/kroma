import type { Status } from '@kromatv/client/live';
import type { SyncState } from '@kromatv/client/party';
import { useT } from '@kromatv/ui';
import { Box, Text } from '@kromatv/ui/kit';

/** Who is watching, and while the viewer is synced, how the group stands. */
export function ViewersCard({
  status,
  syncOn,
  group,
}: Readonly<{ status: Status | null; syncOn: boolean; group: SyncState | null }>) {
  const t = useT();
  if (!status || status.viewers.length === 0) return null;
  const members = group?.members.length ?? 0;
  return (
    <Box absolute right={64} top={150} z={12} pointerEvents="none" align="flex-end">
      <Text variant="labelTv" color="textMuted">
        {t('live.viewers', { count: status.viewers.length })}
      </Text>
      {syncOn ? (
        <Text variant="labelTv" color="accentText">
          {t(members > 1 ? 'live.sync.status.group' : 'live.sync.status.alone', { count: members })}
        </Text>
      ) : null}
    </Box>
  );
}
