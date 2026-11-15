import { describe, expect, it } from 'vitest';
import { SPEAKERS } from './PartyOverlay';

describe('PartyOverlay speakers', () => {
  it('flow inside the overlay wrapper instead of anchoring to its zero-height bottom edge', () => {
    expect(SPEAKERS.position).toBeUndefined();
    expect(SPEAKERS.bottom).toBeUndefined();
  });
});
