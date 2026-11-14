import { describe, expect, it } from 'vitest';
import { REACTIONS, reactedBy, reactionCount } from './reactions';

describe('reactions', () => {
  it('offers the six reactions the server accepts', () => {
    expect(REACTIONS).toHaveLength(6);
    expect(new Set(REACTIONS).size).toBe(6);
  });

  it('counts and attributes a reaction per person', () => {
    const reactions = { x: [{ userId: 'a' }, { userId: 'b' }] };
    expect(reactionCount(reactions, 'x')).toBe(2);
    expect(reactionCount(reactions, 'y')).toBe(0);
    expect(reactedBy(reactions, 'x', 'a')).toBe(true);
    expect(reactedBy(reactions, 'x', 'c')).toBe(false);
  });
});
