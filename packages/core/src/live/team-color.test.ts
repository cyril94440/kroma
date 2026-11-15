import { describe, expect, it } from 'vitest';
import { contrast, readable } from './team-color';

describe('readable', () => {
  it('lightens a dark colour on a dark ground until it reaches 3:1', () => {
    expect(contrast(readable('#000066', '#2e261c'), '#2e261c')).toBeGreaterThanOrEqual(3);
  });

  it('keeps a colour that is already readable', () => {
    expect(readable('#d71431', '#fffdf8')).toBe('#d71431');
  });

  it('refuses anything that is not a plain hex colour', () => {
    expect(readable('red; background:url(x)', '#2e261c')).toMatch(/^#[0-9a-f]{6}$/);
  });
});
