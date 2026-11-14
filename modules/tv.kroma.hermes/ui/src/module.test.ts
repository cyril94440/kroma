import { describe, expect, it } from 'vitest';
import hermesModule from './module';

describe('hermesModule', () => {
  it('takes its identity and dependencies from the shared manifest', () => {
    expect(hermesModule.id).toBe('tv.kroma.hermes');
    expect(hermesModule.dependencies).toEqual({
      'tv.kroma.party': '>=0.1.0',
      'tv.kroma.live': '>=0.1.0',
    });
  });

  it('puts its administration page under system, for administrators', () => {
    expect(hermesModule.routes?.map((r) => r.path)).toEqual(['hermes']);
    expect(hermesModule.navItems).toEqual([
      {
        label: 'nav.hermes',
        icon: 'tv',
        section: 'system',
        requires: 'settings.manage',
        to: '/admin/hermes',
      },
    ]);
  });
});
