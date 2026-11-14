import type { Permission } from '@kromatv/client/accounts';
import { describe, expect, it } from 'vitest';
import { mayWatchNow, offersWatchNow } from './watch-now';

const holding = (...permissions: Permission[]) => ({ permissions });

describe('who may watch a title now', () => {
  it('is a requester whose request needs no approval', () => {
    expect(mayWatchNow(holding('requests.create', 'requests.auto'))).toBe(true);
    expect(mayWatchNow(holding('requests.create', 'requests.manage'))).toBe(true);
  });

  it('is nobody who needs a moderator, or cannot request at all', () => {
    expect(mayWatchNow(holding('requests.create'))).toBe(false);
    expect(mayWatchNow(holding('requests.auto'))).toBe(false);
    expect(mayWatchNow(holding('requests.manage'))).toBe(false);
    expect(mayWatchNow(holding('playback'))).toBe(false);
  });

  it('is nobody who is not signed in', () => {
    expect(mayWatchNow(null)).toBe(false);
    expect(mayWatchNow(undefined)).toBe(false);
  });
});

describe('offering watch now on a title', () => {
  const allowed = holding('requests.create', 'requests.auto');

  it('needs the server to say it can, and the viewer to be allowed', () => {
    expect(offersWatchNow(allowed, true)).toBe(true);
    expect(offersWatchNow(allowed, false)).toBe(false);
    expect(offersWatchNow(holding('requests.create'), true)).toBe(false);
  });
});
