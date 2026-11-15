import { type KromaClient } from '@kromatv/client';
import { KromaEvents } from '@kromatv/client/events';
import type { ViewerHostValue } from '@kromatv/module-sdk';

export function createViewerHost(client: KromaClient, apiBase: string): ViewerHostValue {
  return {
    moduleApi: (id) => client.modules.viewerApi(id),
    fileUrl: (id, path) => client.modules.fileUrl(id, path),
    listModules: () => client.modules.list(),
    openEvents: ({ onEvent, onOpen }) => {
      const events = new KromaEvents<{ type: string }>(apiBase, {
        token: () => client.sessionToken,
        refresh: () => client.refreshSession(),
        onEvent,
        onOpen,
      });
      events.connect();
      return () => events.close();
    },
  };
}
