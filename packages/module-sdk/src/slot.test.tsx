// @vitest-environment jsdom
import { RequestId } from '@kromatv/client/requests';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { AdminHostProvider } from './admin/context';
import { defineModule } from './define';
import { ModuleRegistry } from './registry';
import { useModuleApi } from './scope';
import { ModuleSlot, ModuleSlotProvider, useSlotFilled } from './slot';
import { ViewerHostProvider } from './viewer';

const MANIFEST = { id: 'tv.kroma.acq', version: '1.0.0' };

const REQUEST = RequestId.parse('r1');

function mod(id: string, label: string, order?: number) {
  return defineModule(
    { ...MANIFEST, id },
    {
      slots: [
        {
          slot: 'requests.detail' as const,
          order,
          component: ({ title }: { title: string }) => <span>{`${label}:${title}`}</span>,
        },
      ],
    },
  );
}

function OwnId() {
  const api = useModuleApi() as unknown as { id: string };
  return <span>{api.id}</span>;
}

function mount(registry: ModuleRegistry, enabled: { id: string; enabled?: boolean }[]) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  // The enabled flags come from `GET /api/modules`, which the slot reads through
  // the shared ['modules'] query; seeding it keeps the test off the network.
  queryClient.setQueryData(['modules'], enabled);
  const client = {
    modules: { list: () => Promise.resolve(enabled), api: (id: string) => ({ id }) },
  } as never;
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <AdminHostProvider value={{ client, user: null, apiBase: '' }}>
        <ModuleSlotProvider registry={registry}>{children}</ModuleSlotProvider>
      </AdminHostProvider>
    </QueryClientProvider>
  );
  return render(
    <ModuleSlot
      name="requests.detail"
      requestId={REQUEST}
      kind="show"
      title="Alien"
      canGrab
      fallback={<span>nothing</span>}
    />,
    { wrapper },
  );
}

describe('ModuleSlot', () => {
  it('fills the slot from a module that registers after the slot first rendered', () => {
    const registry = new ModuleRegistry();
    mount(registry, [{ id: 'tv.kroma.acq', enabled: true }]);

    act(() => {
      registry.register(mod('tv.kroma.acq', 'acq'));
    });

    expect(screen.getByText('acq:Alien')).toBeTruthy();
    expect(screen.queryByText('nothing')).toBeNull();
  });

  it('falls back again once the module that filled it unregisters', () => {
    const registry = new ModuleRegistry().register(mod('tv.kroma.acq', 'acq'));
    mount(registry, [{ id: 'tv.kroma.acq', enabled: true }]);

    act(() => {
      registry.unregister('tv.kroma.acq');
    });

    expect(screen.getByText('nothing')).toBeTruthy();
  });

  it('renders what an enabled module contributes, with the page-supplied props', () => {
    const registry = new ModuleRegistry().register(mod('tv.kroma.acq', 'acq'));
    mount(registry, [{ id: 'tv.kroma.acq', enabled: true }]);
    expect(screen.getByText('acq:Alien')).toBeTruthy();
  });

  it('mounts each contribution inside its own module scope', () => {
    const registry = new ModuleRegistry().register(
      defineModule(MANIFEST, { slots: [{ slot: 'requests.detail' as const, component: OwnId }] }),
    );

    mount(registry, [{ id: 'tv.kroma.acq', enabled: true }]);

    expect(screen.getByText('tv.kroma.acq')).toBeTruthy();
  });

  it('falls back when nothing is registered for the slot', () => {
    mount(new ModuleRegistry(), []);
    expect(screen.getByText('nothing')).toBeTruthy();
  });

  it('falls back when the only contributor is disabled', () => {
    const registry = new ModuleRegistry().register(mod('tv.kroma.acq', 'acq'));
    mount(registry, [{ id: 'tv.kroma.acq', enabled: false }]);
    expect(screen.getByText('nothing')).toBeTruthy();
    expect(screen.queryByText('acq:Alien')).toBeNull();
  });

  it('renders several contributors in their declared order', () => {
    const registry = new ModuleRegistry()
      .register(mod('tv.kroma.b', 'b', 2))
      .register(mod('tv.kroma.a', 'a', 1));
    const { container } = mount(registry, [
      { id: 'tv.kroma.a', enabled: true },
      { id: 'tv.kroma.b', enabled: true },
    ]);
    expect(container.textContent).toBe('a:Alienb:Alien');
  });

  it('renders the fallback with no provider at all, rather than throwing', () => {
    const queryClient = new QueryClient();
    queryClient.setQueryData(['modules'], []);
    const client = { modules: { list: () => Promise.resolve([]) } } as never;
    render(
      <QueryClientProvider client={queryClient}>
        <AdminHostProvider value={{ client, user: null, apiBase: '' }}>
          <ModuleSlot
            name="requests.detail"
            requestId={REQUEST}
            kind="movie"
            title="Heat"
            canGrab={false}
            fallback={<span>nothing</span>}
          />
        </AdminHostProvider>
      </QueryClientProvider>,
    );
    expect(screen.getByText('nothing')).toBeTruthy();
  });
});

describe('ModuleSlot defined by a module', () => {
  const NAME = 'tv.kroma.live/player.aside' as const;

  function Aside({ title }: Readonly<{ title: string }>) {
    return <span>{`aside:${title}`}</span>;
  }

  function filler(id: string) {
    return defineModule({ ...MANIFEST, id }, { slots: [{ slot: NAME, component: Aside }] });
  }

  function mountNamed(registry: ModuleRegistry, enabled: { id: string; enabled?: boolean }[]) {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    queryClient.setQueryData(['modules'], enabled);
    const client = { modules: { list: () => Promise.resolve(enabled) } } as never;
    return render(
      <QueryClientProvider client={queryClient}>
        <AdminHostProvider value={{ client, user: null, apiBase: '' }}>
          <ModuleSlotProvider registry={registry}>
            <ModuleSlot name={NAME} title="Match" fallback={<span>empty</span>} />
          </ModuleSlotProvider>
        </AdminHostProvider>
      </QueryClientProvider>,
    );
  }

  it('hands the plain props of the defining page to whichever module fills it', () => {
    mountNamed(new ModuleRegistry().register(filler('tv.kroma.party')), [
      { id: 'tv.kroma.party', enabled: true },
    ]);
    expect(screen.getByText('aside:Match')).toBeTruthy();
  });

  it('falls back when nobody fills it or the filler is off', () => {
    mountNamed(new ModuleRegistry().register(filler('tv.kroma.party')), [
      { id: 'tv.kroma.party', enabled: false },
    ]);
    expect(screen.getByText('empty')).toBeTruthy();
  });

  it('reports whether an enabled module fills it', () => {
    function Probe() {
      return <span>{String(useSlotFilled(NAME))}</span>;
    }
    const registry = new ModuleRegistry().register(filler('tv.kroma.party'));
    const queryClient = new QueryClient();
    queryClient.setQueryData(['modules'], [{ id: 'tv.kroma.party', enabled: true }]);
    const client = { modules: { list: () => Promise.resolve([]) } } as never;
    render(
      <QueryClientProvider client={queryClient}>
        <AdminHostProvider value={{ client, user: null, apiBase: '' }}>
          <ModuleSlotProvider registry={registry}>
            <Probe />
          </ModuleSlotProvider>
        </AdminHostProvider>
      </QueryClientProvider>,
    );
    expect(screen.getByText('true')).toBeTruthy();
  });

  it('contains a filler that throws, so the defining page survives', () => {
    const quiet = vi.spyOn(console, 'error').mockImplementation(() => {});
    function Boom(): never {
      throw new Error('boom');
    }
    const registry = new ModuleRegistry().register(
      defineModule(
        { ...MANIFEST, id: 'tv.kroma.party' },
        { slots: [{ slot: NAME, component: Boom }] },
      ),
    );
    const { container } = mountNamed(registry, [{ id: 'tv.kroma.party', enabled: true }]);
    expect(container.textContent).toBe('');
    quiet.mockRestore();
  });

  it('works in a shell with only the viewer host, where the module list comes from it', async () => {
    const registry = new ModuleRegistry().register(filler('tv.kroma.party'));
    const queryClient = new QueryClient();
    const listModules = vi.fn(() => Promise.resolve([{ id: 'tv.kroma.party', enabled: true }]));
    render(
      <QueryClientProvider client={queryClient}>
        <ViewerHostProvider
          value={{
            moduleApi: () => ({}) as never,
            fileUrl: () => '',
            listModules: listModules as never,
            openEvents: () => () => {},
          }}
        >
          <ModuleSlotProvider registry={registry}>
            <ModuleSlot name={NAME} title="Match" fallback={<span>empty</span>} />
          </ModuleSlotProvider>
        </ViewerHostProvider>
      </QueryClientProvider>,
    );
    expect(await screen.findByText('aside:Match')).toBeTruthy();
    expect(listModules).toHaveBeenCalled();
  });
});
