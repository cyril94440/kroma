import { describe, expect, it } from 'vitest';
import partyModule from './module';

describe('partyModule', () => {
  it('takes its identity from the shared manifest and depends on the live module', () => {
    expect(partyModule.id).toBe('tv.kroma.party');
    expect(partyModule.dependencies).toEqual({ 'tv.kroma.live': '>=0.1.0' });
  });

  it('puts its page in the main sidebar rather than the admin console', () => {
    expect(partyModule.routes?.map((r) => r.path)).toEqual(['party']);
    expect(partyModule.navItems).toEqual([
      { label: 'nav.party', icon: 'tv', section: 'library', to: '/party' },
    ]);
  });
});
