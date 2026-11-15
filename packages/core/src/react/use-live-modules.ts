import type { KromaClient } from '@kromatv/client';
import { useEffect, useState } from 'react';
import { liveModules, type LiveModules, NO_LIVE_MODULES } from '../live/modules';

export interface LiveModulesState {
  modules: LiveModules;
  ready: boolean;
}

/** Asks the server which live-TV modules it runs. Until the answer arrives, and
 *  when it never does, every module reads as absent, so a screen gated on it
 *  never shows for a server that has not said it can. */
const ABSENT: LiveModulesState = { modules: NO_LIVE_MODULES, ready: false };

export function useLiveModules(client: Pick<KromaClient, 'modules'> | null): LiveModulesState {
  const [state, setState] = useState<LiveModulesState>(ABSENT);
  useEffect(() => {
    if (!client) {
      setState(ABSENT);
      return;
    }
    let cancelled = false;
    client.modules.list().then(
      (list) => {
        if (!cancelled) setState({ modules: liveModules(list), ready: true });
      },
      () => {
        if (!cancelled) setState({ modules: NO_LIVE_MODULES, ready: true });
      },
    );
    return () => {
      cancelled = true;
    };
  }, [client]);
  return state;
}
