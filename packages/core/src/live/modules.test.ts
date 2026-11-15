import type { ModuleInfo } from '@kromatv/client/modules';
import { describe, expect, it } from 'vitest';
import { liveModules, NO_LIVE_MODULES } from './modules';

const info = (id: string, enabled?: boolean) => ({ id, name: id, enabled }) as ModuleInfo;

describe('which live modules a server runs', () => {
  it('finds none on a server without them', () => {
    expect(liveModules([info('tv.kroma.torrents')])).toEqual(NO_LIVE_MODULES);
    expect(liveModules([])).toEqual(NO_LIVE_MODULES);
  });

  it('counts a module whose enabled flag is absent as enabled', () => {
    expect(liveModules([info('tv.kroma.live')]).live).toBe(true);
  });

  it('ignores a module an admin switched off', () => {
    const found = liveModules([info('tv.kroma.live', false), info('tv.kroma.party', true)]);

    expect(found).toEqual({ live: false, party: true, sport: false });
  });
});
