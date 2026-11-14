import { afterEach, describe, expect, it, vi } from 'vitest';
import { probeSourceFailure, readSourceFailure } from './source-failure';

const answer = (status: number, body: unknown) =>
  new Response(typeof body === 'string' ? body : JSON.stringify(body), { status });

afterEach(() => vi.unstubAllGlobals());

describe('readSourceFailure', () => {
  it.each([
    [502, 'error.sourceRefused'],
    [502, 'error.sourceUnreachable'],
    [502, 'error.sourceGone'],
    [503, 'error.sourceDisabled'],
  ])('names the source failure a %i answer carries: %s', async (status, code) => {
    expect(await readSourceFailure(answer(status, { error: 'x', code }))).toEqual({
      messageKey: code,
    });
  });

  it('shows the server message for an error code this client has no key for', async () => {
    expect(
      await readSourceFailure(
        answer(503, { error: 'Still being prepared', code: 'error.notCached' }),
      ),
    ).toEqual({
      messageKey: 'error.sourceMessage',
      messageVars: { message: 'Still being prepared' },
    });
  });

  it('falls back to a generic line for an unknown code with no message', async () => {
    expect(await readSourceFailure(answer(502, { code: 'error.notCached' }))).toEqual({
      messageKey: 'error.sourceUnknown',
    });
  });

  it('ignores a body whose code is not an error key', async () => {
    expect(await readSourceFailure(answer(502, { error: 'x', code: 'oops' }))).toBeNull();
    expect(await readSourceFailure(answer(502, { error: 'x' }))).toBeNull();
    expect(await readSourceFailure(answer(502, { code: 7 }))).toBeNull();
  });

  it('ignores a status that is not a source failure, whatever the body says', async () => {
    expect(
      await readSourceFailure(answer(500, { error: 'x', code: 'error.sourceRefused' })),
    ).toBeNull();
  });

  it('ignores a body that is not JSON', async () => {
    expect(await readSourceFailure(answer(502, '<html>bad gateway</html>'))).toBeNull();
  });
});

describe('probeSourceFailure', () => {
  it('asks for one byte and reports the refusal', async () => {
    const fetch = vi
      .fn()
      .mockResolvedValue(answer(502, { error: 'x', code: 'error.sourceRefused' }));
    vi.stubGlobal('fetch', fetch);

    expect(await probeSourceFailure('/stream')).toEqual({ messageKey: 'error.sourceRefused' });
    expect(fetch).toHaveBeenCalledWith('/stream', { headers: { Range: 'bytes=0-0' } });
  });

  it('reports nothing when the request itself fails', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('offline')));

    expect(await probeSourceFailure('/stream')).toBeNull();
  });
});
