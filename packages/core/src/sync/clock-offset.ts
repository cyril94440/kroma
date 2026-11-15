export interface ClockSample {
  rtt: number;
  offset: number;
}

/** Estimates the offset between this clock and the server's from one request:
 *  the server answered at its `serverNow` somewhere between `sentAt` and
 *  `receivedAt`, so it is taken at the midpoint. */
export function sample(sentAt: number, serverNow: number, receivedAt: number): ClockSample {
  return { rtt: receivedAt - sentAt, offset: serverNow - (sentAt + receivedAt) / 2 };
}

/** The sample with the smallest round trip, which bounds the error the most.
 *  The precision is half that round trip: about 5 to 30 ms on a LAN and tens of
 *  milliseconds over the internet, against the 500 ms skew it corrects. */
export function bestOffset(samples: readonly ClockSample[]): ClockSample | null {
  return samples.reduce<ClockSample | null>(
    (best, s) => (best === null || s.rtt < best.rtt ? s : best),
    null,
  );
}

/** A burst of `count` time requests, the best of them kept. */
export async function measureOffset(
  fetchServerNow: () => Promise<number>,
  count = 8,
  clock: () => number = Date.now,
  gapMs = 0,
): Promise<ClockSample | null> {
  const samples: ClockSample[] = [];
  for (let i = 0; i < count; i += 1) {
    if (i > 0 && gapMs > 0) await new Promise((resolve) => setTimeout(resolve, gapMs));
    const sentAt = clock();
    try {
      const serverNow = await fetchServerNow();
      samples.push(sample(sentAt, serverNow, clock()));
    } catch {
      break;
    }
  }
  return bestOffset(samples);
}
