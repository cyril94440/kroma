import type { KromaClient } from '@kromatv/client';
import { useLiveModules } from '@kromatv/core/react';
import { type ReactNode, useMemo } from 'react';
import { useAuth } from '#tv/app/providers/auth';
import { LiveModulesProvider } from '#tv/shared/nav/liveModules';
import { type NavEntry, NavEntriesProvider } from '#tv/shared/nav/navEntries';

const LIVE_ENTRY: NavEntry = { key: 'live', icon: 'broadcast', label: 'nav.live', route: 'live' };

/** Asks the signed-in server which live-TV modules it runs and puts the answer
 *  where the screens read it. The Direct entry exists only when the live module
 *  is enabled and this shell can play it. */
export function LiveGate({
  client,
  available,
  children,
}: Readonly<{ client: KromaClient | null; available: boolean; children: ReactNode }>): ReactNode {
  const { user, ready } = useAuth();
  const state = useLiveModules(ready && user ? client : null);
  const entries = useMemo(
    () => (available && state.modules.live ? [LIVE_ENTRY] : []),
    [available, state.modules.live],
  );
  return (
    <LiveModulesProvider value={state}>
      <NavEntriesProvider entries={entries}>{children}</NavEntriesProvider>
    </LiveModulesProvider>
  );
}
