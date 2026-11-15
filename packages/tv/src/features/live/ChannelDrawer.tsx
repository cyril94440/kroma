import type { ChannelView } from '@kromatv/client/live';
import { useT } from '@kromatv/ui';
import { Drawer, FocusColumn, ListRow, Text } from '@kromatv/ui/kit';
import { useChannelList } from '#tv/features/live/useChannelList';
import { useClient } from '#tv/app/router';

function ChannelRow({
  channel,
  current,
  first,
  onPick,
}: Readonly<{
  channel: ChannelView;
  current: boolean;
  first: boolean;
  onPick: (channel: ChannelView) => void;
}>) {
  return (
    <ListRow.Root
      icon="device-tv"
      size="tv"
      ground="surface"
      chevron={false}
      selected={current}
      autoFocus={first}
      onPress={() => onPick(channel)}
    >
      <ListRow.Label>{channel.name}</ListRow.Label>
      {channel.now ? <ListRow.Hint>{channel.now.title}</ListRow.Hint> : null}
    </ListRow.Root>
  );
}

/** The channel list a zap starts from: the quick-access channels first, then
 *  every channel a page at a time. Choosing one hands it to `onPick`. */
export function ChannelDrawer({
  open,
  currentId,
  onClose,
  onPick,
}: Readonly<{
  open: boolean;
  currentId: string | null;
  onClose: () => void;
  onPick: (channel: ChannelView) => void;
}>) {
  const t = useT();
  const client = useClient();
  const list = useChannelList(client, open);
  const favouriteIds = new Set(list.favourites.map((channel) => channel.id));
  const rest = list.channels.filter((channel) => !favouriteIds.has(channel.id));
  return (
    <Drawer.Root open={open} onClose={onClose} title={t('live.channels')} width="md" floating>
      <Drawer.Panel>
        <FocusColumn>
          {list.favourites.length > 0 ? (
            <Text variant="overlineTv" color="accentText">
              {t('live.favourites')}
            </Text>
          ) : null}
          {list.favourites.map((channel, at) => (
            <ChannelRow
              key={`fav-${channel.id}`}
              channel={channel}
              current={channel.id === currentId}
              first={at === 0}
              onPick={onPick}
            />
          ))}
          {rest.length > 0 ? (
            <Text variant="overlineTv" color="accentText">
              {t('live.allChannels')}
            </Text>
          ) : null}
          {rest.map((channel, at) => (
            <ChannelRow
              key={channel.id}
              channel={channel}
              current={channel.id === currentId}
              first={list.favourites.length === 0 && at === 0}
              onPick={onPick}
            />
          ))}
          {list.hasMore ? (
            <ListRow.Root
              icon="chevron-down"
              size="tv"
              ground="surface"
              chevron={false}
              onPress={list.loadMore}
            >
              <ListRow.Label>{t('live.more')}</ListRow.Label>
            </ListRow.Root>
          ) : null}
          {list.favourites.length + list.channels.length === 0 ? (
            <Text variant="bodyTv" color="textDim">
              {t('live.noChannels')}
            </Text>
          ) : null}
        </FocusColumn>
      </Drawer.Panel>
    </Drawer.Root>
  );
}
