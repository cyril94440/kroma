// @vitest-environment jsdom
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useLiveModulesValue } from '#tv/shared/nav/liveModules';
import { useNavEntries } from '#tv/shared/nav/navEntries';

let signedIn = true;
let ready = true;
vi.mock('#tv/app/providers/auth', () => ({
  useAuth: () => ({ user: signedIn ? { id: 'u1' } : null, ready }),
}));

const { LiveGate } = await import('#tv/features/live/LiveGate');

afterEach(() => {
  cleanup();
  signedIn = true;
  ready = true;
});

const clientWith = (ids: { id: string; enabled?: boolean }[]) => {
  const list = vi.fn(async () => ids.map((m) => ({ ...m, name: m.id })));
  return { client: { baseUrl: 'http://s', modules: { list } } as never, list };
};

function Probe() {
  const entries = useNavEntries();
  const { modules } = useLiveModulesValue();
  return (
    <span data-testid="probe">{`${entries.map((e) => e.route).join(',')}|${modules.party}`}</span>
  );
}

const probe = () => screen.getByTestId('probe').textContent;

describe('the live gate', () => {
  it('offers no entry on a server without the live module', async () => {
    const { client } = clientWith([{ id: 'tv.kroma.party' }]);
    render(
      <LiveGate client={client} available>
        <Probe />
      </LiveGate>,
    );

    await waitFor(() => expect(probe()).toBe('|true'));
  });

  it('offers the live entry when the module is enabled and the shell can play it', async () => {
    const { client } = clientWith([{ id: 'tv.kroma.live' }]);
    render(
      <LiveGate client={client} available>
        <Probe />
      </LiveGate>,
    );

    await waitFor(() => expect(probe()).toBe('live|false'));
  });

  it('offers none when the module is switched off or the shell has no player for it', async () => {
    const { client: off, list: offList } = clientWith([{ id: 'tv.kroma.live', enabled: false }]);
    const first = render(
      <LiveGate client={off} available>
        <Probe />
      </LiveGate>,
    );
    await waitFor(() => expect(offList).toHaveBeenCalled());
    expect(probe()).toBe('|false');
    first.unmount();

    const { client: on, list: onList } = clientWith([{ id: 'tv.kroma.live' }]);
    render(
      <LiveGate client={on} available={false}>
        <Probe />
      </LiveGate>,
    );
    await waitFor(() => expect(onList).toHaveBeenCalled());
    expect(probe()).toBe('|false');
  });

  it('asks nothing until the session has been resumed, so the first request carries a bearer', () => {
    ready = false;
    const { client, list } = clientWith([{ id: 'tv.kroma.live' }]);
    render(
      <LiveGate client={client} available>
        <Probe />
      </LiveGate>,
    );

    expect(list).not.toHaveBeenCalled();
  });

  it('asks nothing while signed out', async () => {
    signedIn = false;
    const { client, list } = clientWith([{ id: 'tv.kroma.live' }]);
    render(
      <LiveGate client={client} available>
        <Probe />
      </LiveGate>,
    );

    expect(list).not.toHaveBeenCalled();
    expect(probe()).toBe('|false');
  });
});
