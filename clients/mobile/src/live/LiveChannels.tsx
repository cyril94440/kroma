import type { ChannelView } from '@kromatv/client/live';
import { Box, Button, Text } from '@kromatv/ui/kit';
import { useEffect, useState } from 'react';
import { useT } from '#mobile/lib/i18n';
import type { LiveClient } from '@kromatv/core/react';

/** The quick-access channels, then every channel, each one a row that asks for
 *  a zap: the vote decides on a family server, the server on a solo one. */
export function LiveChannels({
  client,
  currentId,
  onPick,
}: Readonly<{
  client: Pick<LiveClient, 'live'>;
  currentId: string | null;
  onPick: (channel: ChannelView) => void;
}>) {
  const t = useT();
  const [favourites, setFavourites] = useState<ChannelView[]>([]);
  const [channels, setChannels] = useState<ChannelView[]>([]);
  useEffect(() => {
    let cancelled = false;
    client.live.favourites().then(
      (list) => !cancelled && setFavourites(list),
      () => undefined,
    );
    client.live.channels({ perPage: 40 }).then(
      (page) => !cancelled && setChannels(page.channels),
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [client]);
  const known = new Set(favourites.map((channel) => channel.id));
  const rows = [...favourites, ...channels.filter((channel) => !known.has(channel.id))];
  return (
    <Box gap={8}>
      {rows.length === 0 ? (
        <Text variant="body" color="textDim">
          {t('live.noChannels')}
        </Text>
      ) : null}
      {rows.map((channel) => (
        <Button
          key={channel.id}
          icon="device-tv"
          label={channel.now ? `${channel.name}  ·  ${channel.now.title}` : channel.name}
          variant={channel.id === currentId ? 'primary' : 'scrim'}
          onPress={() => onPick(channel)}
        />
      ))}
    </Box>
  );
}
