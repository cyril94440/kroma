import { describe, expect, it } from 'vitest';
import sportModule from './module';

describe('sportModule', () => {
  it('takes its identity from the shared manifest and depends on the live module', () => {
    expect(sportModule.id).toBe('tv.kroma.sport');
    expect(sportModule.dependencies).toEqual({ 'tv.kroma.live': '>=0.1.0' });
  });

  it('fills the live player aside slot with the compact match panel', () => {
    expect(sportModule.slots?.map((s) => s.slot)).toEqual(['tv.kroma.live/player.aside']);
  });

  it('puts its page in the main sidebar', () => {
    expect(sportModule.routes?.map((r) => r.path)).toEqual(['sport']);
    expect(sportModule.navItems).toEqual([
      { label: 'sport.nav', icon: 'chart', section: 'library', to: '/sport' },
    ]);
  });
});
