import type { SyncState } from '@kromatv/client/party';
import type { LiveSync as Sync } from '@kromatv/core/react';
import { Badge, Box, Button, Text } from '@kromatv/ui/kit';
import { useT } from '#mobile/lib/i18n';

/** Joins or leaves the watch party and says where the sync stands. */
export function LiveSyncPanel({ sync, group }: Readonly<{ sync: Sync; group: SyncState | null }>) {
  const t = useT();
  const members = group?.members.length ?? 0;
  let status = t('live.sync.status.off');
  if (sync.on) {
    status =
      members > 1 ? t('live.sync.status.group', { count: members }) : t('live.sync.status.alone');
  }
  return (
    <Box gap={6} p={16} radius="lg" bg="surface2">
      <Box row gap={10} align="center">
        <Button
          icon="users-group"
          label={t(sync.on ? 'live.sync.leave' : 'live.sync.join')}
          variant={sync.on ? 'scrim' : 'primary'}
          onPress={sync.on ? sync.leave : sync.join}
        />
        {sync.on ? <Badge>{t('live.sync.badge')}</Badge> : null}
      </Box>
      <Text variant="meta" color="textMuted">
        {group?.by && sync.on
          ? `${status} · ${t('live.sync.by', { name: group.by.name })}`
          : status}
      </Text>
    </Box>
  );
}
