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
    expect(await readSourceFailure(answer(status, { error: 'x', code }))).toBe(code);
  });

  it('ignores a code the catalog does not hold', async () => {
    expect(await readSourceFailure(answer(502, { error: 'x', code: 'error.other' }))).toBeNull();
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

    expect(await probeSourceFailure('/stream')).toBe('error.sourceRefused');
    expect(fetch).toHaveBeenCalledWith('/stream', { headers: { Range: 'bytes=0-0' } });
  });

  it('reports nothing when the request itself fails', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('offline')));

    expect(await probeSourceFailure('/stream')).toBeNull();
  });
});
