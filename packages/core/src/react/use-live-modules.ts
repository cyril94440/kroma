import type { KromaClient } from '@kromatv/client';
import { useEffect, useState } from 'react';
import { liveModules, type LiveModules, NO_LIVE_MODULES } from '../live/modules';

export interface LiveModulesState {
  modules: LiveModules;
  ready: boolean;
}

type Client = Pick<KromaClient, 'modules'>;

const ABSENT: LiveModulesState = { modules: NO_LIVE_MODULES, ready: false };

/** Asks the server which live-TV modules it runs. Until the answer arrives, for
 *  another client than the one that answered, and when it never does, every
 *  module reads as absent, so a screen gated on it never shows for a server
 *  that has not said it can. */
export function useLiveModules(client: Client | null): LiveModulesState {
  const [answer, setAnswer] = useState<{ client: Client; state: LiveModulesState } | null>(null);
  useEffect(() => {
    if (!client) return;
    let cancelled = false;
    const settle = (state: LiveModulesState) => {
      if (!cancelled) setAnswer({ client, state });
    };
    client.modules.list().then(
      (list) => settle({ modules: liveModules(list), ready: true }),
      () => settle({ modules: NO_LIVE_MODULES, ready: true }),
    );
    return () => {
      cancelled = true;
    };
  }, [client]);
  return client && answer?.client === client ? answer.state : ABSENT;
}
