import { describe, expect, it } from 'vitest';
import { clipHref, clipRoute } from './clip';

const clip = {
  uri: 'http://s/api/module-files/x?t=a&b=1',
  title: "37' Kebbal",
  subtitle: 'Rennes 1-0 OM',
};

describe('the clip route', () => {
  it('round-trips the file and its caption through the address', () => {
    const query = new URLSearchParams(clipHref(clip).split('?')[1]);

    expect(
      clipRoute({
        uri: query.get('uri') ?? undefined,
        title: query.get('title') ?? undefined,
        subtitle: query.get('subtitle') ?? undefined,
      }),
    ).toEqual(clip);
  });

  it('refuses an address that names no file', () => {
    expect(clipRoute({ title: 'x', subtitle: 'y' })).toBeNull();
    expect(clipRoute({ uri: 'undefined', title: 'x', subtitle: 'y' })).toBeNull();
  });
});
