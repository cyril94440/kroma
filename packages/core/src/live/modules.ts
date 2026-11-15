import type { ModuleInfo } from '@kromatv/client/modules';
import { LIVE_MODULE } from '@kromatv/client/live';
import { PARTY_MODULE } from '@kromatv/client/party';
import { SPORT_MODULE } from '@kromatv/client/sport';

export interface LiveModules {
  live: boolean;
  party: boolean;
  sport: boolean;
}

export const NO_LIVE_MODULES: LiveModules = { live: false, party: false, sport: false };

function enabled(modules: readonly ModuleInfo[], id: string): boolean {
  return modules.some((module) => module.id === id && module.enabled !== false);
}

/** Which of the live-TV modules this server runs and has enabled. The live
 *  screens exist only when `live` is set; `party` and `sport` add their panels. */
export function liveModules(modules: readonly ModuleInfo[]): LiveModules {
  return {
    live: enabled(modules, LIVE_MODULE),
    party: enabled(modules, PARTY_MODULE),
    sport: enabled(modules, SPORT_MODULE),
  };
}
