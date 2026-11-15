import { describe, expect, it } from 'vitest';
import { isHiddenTab, tabOptions } from './hiddenTab';

describe('a tab that exists only when the server has its module', () => {
  it('is hidden from the bar when the module is absent', () => {
    expect(isHiddenTab(tabOptions(false))).toBe(true);
  });

  it('is in the bar when the module is there', () => {
    expect(isHiddenTab(tabOptions(true))).toBe(false);
  });

  it('is in the bar when it carries no style at all', () => {
    expect(isHiddenTab({})).toBe(false);
  });
});
