import type { IconName } from '@kromatv/ui/kit';
import { createContext, type ReactNode, useContext } from 'react';
import type { RouteName } from '#tv/app/router';

export interface NavEntry {
  key: string;
  icon: IconName;
  label: 'nav.live';
  route: Extract<RouteName, 'live'>;
}

const NavEntriesContext = createContext<readonly NavEntry[]>([]);

/** The top bar's entries that exist only when something on the server makes
 *  them true. The app decides which, so the bar names no feature. */
export function NavEntriesProvider({
  entries,
  children,
}: Readonly<{ entries: readonly NavEntry[]; children: ReactNode }>): ReactNode {
  return <NavEntriesContext value={entries}>{children}</NavEntriesContext>;
}

export function useNavEntries(): readonly NavEntry[] {
  return useContext(NavEntriesContext);
}
