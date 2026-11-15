import type { ModuleApi } from '@kromatv/module-sdk';
import { describe, expect, it, vi } from 'vitest';
import { createSportApi } from './sport-api';

function fake() {
  const get = vi.fn(async (_path: string, schema: { parse: (v: unknown) => unknown }) =>
    schema.parse({}),
  );
  const post = vi.fn(
    async (_path: string, _body?: unknown, schema?: { parse: (v: unknown) => unknown }) =>
      schema?.parse({}),
  );
  return { get, post, api: { get, post } as unknown as ModuleApi };
}

describe('createSportApi', () => {
  it('posts a prediction with the match id and the score', async () => {
    const { api, post } = fake();

    await createSportApi(api).vote('espn-1', '2-1');

    expect(post).toHaveBeenCalledWith(
      '/prono/vote',
      { id: 'espn-1', score: '2-1' },
      expect.anything(),
    );
  });

  it('encodes the competition of a standings read', async () => {
    const { api, get } = fake();

    await createSportApi(api).standings('Ligue 1');

    expect(get).toHaveBeenCalledWith('/standings?competition=Ligue%201', expect.anything());
  });

  it('sends the clock read on the picture with the frame time it was read at', async () => {
    const { api, post } = fake();

    await createSportApi(api).calibrate('52:13', 1_800_000_000_000);

    expect(post).toHaveBeenCalledWith(
      '/calibration',
      { clock: '52:13', capturedAtMs: 1_800_000_000_000 },
      expect.anything(),
    );
  });

  it('asks for a watch on the channel of a match through the room', async () => {
    const { api, post } = fake();

    await createSportApi(api)
      .watch('primary:l1p1')
      .catch(() => undefined);

    expect(post).toHaveBeenCalledWith('/watch', { channelId: 'primary:l1p1' }, expect.anything());
  });
});
