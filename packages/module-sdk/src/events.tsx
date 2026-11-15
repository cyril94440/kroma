// Live server events for module pages: one socket per mount, torn down with
// the component, handler always the latest render's.

import { KromaEvents, type ServerEvent } from '@kromatv/client/events';
import { useEffect, useRef } from 'react';
import { useOptionalAdminHost } from './admin/context';
import { useOptionalViewerHost } from './viewer';

/** Subscribe to the server's event socket, in the admin console or in a page
 *  the main shell mounts. A module widens the frame union with its own event
 *  types, declared in its package, not in core:
 *
 *  ```ts
 *  useServerEvents<DownloadProgressEvent | VpnStatusEvent>((e) => { ... });
 *  ```
 *
 *  `onOpen` runs on every (re)connect, the moment to re-read state missed while
 *  the socket was down.
 */
export function useServerEvents<M extends { type: string } = never>(
  onEvent: (e: ServerEvent | M) => void,
  onOpen?: () => void,
): void {
  const admin = useOptionalAdminHost();
  const viewer = useOptionalViewerHost();
  const handlers = useRef({ onEvent, onOpen });
  useEffect(() => {
    handlers.current = { onEvent, onOpen };
  });
  useEffect(() => {
    const forward = (e: ServerEvent | M) => handlers.current.onEvent(e);
    const opened = () => handlers.current.onOpen?.();
    if (admin) {
      const ev = new KromaEvents<ServerEvent | M>(admin.apiBase, {
        token: () => admin.client.sessionToken,
        refresh: () => admin.client.refreshSession(),
        onEvent: forward,
        onOpen: opened,
      });
      ev.connect();
      return () => ev.close();
    }
    if (!viewer) throw new Error('useServerEvents needs the admin shell or a ViewerHostProvider');
    return viewer.openEvents({ onEvent: (e) => forward(e as ServerEvent | M), onOpen: opened });
  }, [admin, viewer]);
}
