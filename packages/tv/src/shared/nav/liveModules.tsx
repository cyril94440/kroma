import { type LiveModules, NO_LIVE_MODULES } from '@kromatv/core';
import { createContext, type ReactNode, useContext } from 'react';

interface LiveModulesValue {
  modules: LiveModules;
  ready: boolean;
}

const LiveModulesContext = createContext<LiveModulesValue>({
  modules: NO_LIVE_MODULES,
  ready: false,
});

export function LiveModulesProvider({
  value,
  children,
}: Readonly<{ value: LiveModulesValue; children: ReactNode }>): ReactNode {
  return <LiveModulesContext value={value}>{children}</LiveModulesContext>;
}

export function useLiveModulesValue(): LiveModulesValue {
  return useContext(LiveModulesContext);
}
