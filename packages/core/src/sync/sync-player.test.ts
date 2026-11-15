import { describe, expect, it } from 'vitest';
import { clampMove } from './sync-player';

describe('clampMove', () => {
  const range = { backMs: 5000, forwardMs: 2000 };

  it('leaves a move inside the range alone', () => {
    expect(clampMove(1000, range)).toBe(1000);
    expect(clampMove(-4000, range)).toBe(-4000);
  });

  it('limits a move to what the window allows, in both directions', () => {
    expect(clampMove(9000, range)).toBe(2000);
    expect(clampMove(-9000, range)).toBe(-5000);
  });

  it('forbids a forward move past the safe zone but never turns a backward one into a bigger jump', () => {
    const past = { backMs: 5000, forwardMs: -1500 };
    expect(clampMove(1000, past)).toBe(0);
    expect(clampMove(-150, past)).toBe(-150);
  });

  it('trusts the move when the window is unknown', () => {
    expect(clampMove(-7000, null)).toBe(-7000);
  });
});
