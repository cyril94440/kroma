import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Timers } from './voice-timing';

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('Timers', () => {
  it('runs a named timer once and forgets it', () => {
    const timers = new Timers();
    const run = vi.fn();

    timers.arm('a', 100, run);
    vi.advanceTimersByTime(100);

    expect(run).toHaveBeenCalledTimes(1);
    expect(timers.has('a')).toBe(false);
  });

  it('replaces a timer armed again under the same name', () => {
    const timers = new Timers();
    const first = vi.fn();
    const second = vi.fn();

    timers.arm('a', 100, first);
    timers.arm('a', 100, second);
    vi.advanceTimersByTime(100);

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalled();
  });

  it('clears one timer or all of them', () => {
    const timers = new Timers();
    const run = vi.fn();
    timers.arm('a', 100, run);
    timers.arm('b', 100, run);
    timers.arm('c', 100, run);

    timers.clear('a');
    timers.clearAll();
    vi.advanceTimersByTime(100);

    expect(run).not.toHaveBeenCalled();
  });
});
