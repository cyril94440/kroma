import { playlistKey } from '@kromatv/core';
import { type LiveRoom, socketEvents, useLiveRoom } from '@kromatv/core/react';
import { useForeground } from '@kromatv/ui';
import { useMemo } from 'react';
import { useClient } from '#tv/app/router';
import { useLiveModulesValue } from '#tv/shared/nav/liveModules';

interface LiveScreen {
  room: LiveRoom;
  playlistUrl: string | null;
  modules: ReturnType<typeof useLiveModulesValue>['modules'];
  modulesReady: boolean;
}

/** Everything the live screen follows: the room (status, party, sport over one
 *  socket, presence while the app is in front) and the ticketed playlist URL,
 *  which changes only when the stream restarts under a new folder. */
export function useLiveScreen(): LiveScreen {
  const client = useClient();
  const { modules, ready } = useLiveModulesValue();
  const foreground = useForeground();
  const openEvents = useMemo(() => socketEvents(client), [client]);
  const room = useLiveRoom({ client, openEvents, modules, watching: foreground && ready });
  const playlist = playlistKey(room.status);
  const playlistUrl = useMemo(
    () => (playlist ? client.live.playlistUrl(playlist) : null),
    [client, playlist],
  );
  return { room, playlistUrl, modules, modulesReady: ready };
}
