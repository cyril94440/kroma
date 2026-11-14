import { describe, expect, it } from 'vitest';
import { bestOffset, measureOffset, sample } from './clock-offset';

describe('clock offset', () => {
  it('takes the server instant at the midpoint of the round trip', () => {
    expect(sample(1000, 6050, 1100)).toEqual({ rtt: 100, offset: 5000 });
  });

  it('keeps the sample with the smallest round trip', () => {
    const best = bestOffset([
      { rtt: 80, offset: 510 },
      { rtt: 12, offset: 498 },
      { rtt: 40, offset: 505 },
    ]);
    expect(best).toEqual({ rtt: 12, offset: 498 });
    expect(bestOffset([])).toBeNull();
  });

  it('measures a burst against a server running 500 ms ahead', async () => {
    let local = 10_000;
    const latencies = [30, 8, 50, 12];
    let i = 0;
    const result = await measureOffset(
      async () => {
        const rtt = latencies[i % latencies.length] ?? 0;
        i += 1;
        local += rtt / 2;
        const server = local + 500;
        local += rtt / 2;
        return server;
      },
      4,
      () => local,
    );
    expect(result?.rtt).toBe(8);
    expect(result?.offset).toBeCloseTo(500, 5);
  });

  it('returns what it has when a request fails mid-burst', async () => {
    let calls = 0;
    const result = await measureOffset(async () => {
      calls += 1;
      if (calls === 2) throw new Error('offline');
      return 1000;
    }, 5);
    expect(calls).toBe(2);
    expect(result).not.toBeNull();
  });
});
