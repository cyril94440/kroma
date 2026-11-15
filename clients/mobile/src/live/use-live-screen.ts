import { socketEvents, useLiveModules, useLiveRoom } from '@kromatv/core/react';
import { useForeground } from '@kromatv/ui';
import { useFocusEffect } from 'expo-router';
import { useCallback, useMemo, useState } from 'react';
import { useClient } from '#mobile/lib/session';

/** The live room of the signed-in phone: status, party and sport over one
 *  socket. The viewer counts as present only while this screen is in front and
 *  the app is in the foreground, so backgrounding the app says goodbye. */
export function useLiveScreen() {
  const client = useClient();
  const { modules, ready } = useLiveModules(client);
  const foreground = useForeground();
  const [focused, setFocused] = useState(false);
  useFocusEffect(
    useCallback(() => {
      setFocused(true);
      return () => setFocused(false);
    }, []),
  );
  const openEvents = useMemo(() => socketEvents(client), [client]);
  const room = useLiveRoom({
    client,
    openEvents,
    modules,
    watching: foreground && focused && ready,
  });
  return { client, room, modules, ready };
}
