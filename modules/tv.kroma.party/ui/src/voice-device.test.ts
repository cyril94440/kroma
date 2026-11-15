import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { deviceId, newSessionId } from './voice-device';

function storage() {
  const values = new Map<string, string>();
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
  };
}

beforeEach(() => {
  vi.stubGlobal('sessionStorage', storage());
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('deviceId', () => {
  it('is a token the module accepts and stays the same for a tab', () => {
    const first = deviceId();

    expect(first).toMatch(/^[a-f0-9]{24}$/);
    expect(deviceId()).toBe(first);
  });

  it('falls back to a fresh id when storage is unavailable', () => {
    vi.stubGlobal('sessionStorage', {
      getItem: () => {
        throw new Error('blocked');
      },
      setItem: () => undefined,
    });

    expect(deviceId()).toMatch(/^[a-f0-9]{24}$/);
  });
});

describe('newSessionId', () => {
  it('is a short token and differs each time', () => {
    const [a, b] = [newSessionId(), newSessionId()];

    expect(a).toMatch(/^[a-f0-9]{12}$/);
    expect(a).not.toBe(b);
  });
});
