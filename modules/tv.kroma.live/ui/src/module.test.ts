import { describe, expect, it } from 'vitest';
import liveModule from './module';

describe('liveModule', () => {
  it('takes its identity from the shared manifest', () => {
    expect(liveModule.id).toBe('tv.kroma.live');
  });

  it('puts the watch page in the main sidebar and the administration under acquisition', () => {
    expect(liveModule.routes?.map((r) => r.path)).toEqual(['live', 'live-admin']);
    expect(liveModule.navItems?.map((n) => n.section)).toEqual(['library', 'acquisition']);
  });
});
