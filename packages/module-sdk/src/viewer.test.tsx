// @vitest-environment jsdom
import { ModuleId } from '@kromatv/client/modules';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, cleanup, render, renderHook, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ModuleScope } from './scope';
import { useServerEvents } from './events';
import {
  useModuleFileUrl,
  useViewerApi,
  type ViewerEventHandlers,
  ViewerHostProvider,
  type ViewerHostValue,
} from './viewer';

afterEach(cleanup);

function host(overrides: Partial<ViewerHostValue> = {}): ViewerHostValue {
  return {
    moduleApi: (id) => ({ id }) as never,
    fileUrl: (id, path) => `/files/${id}/${path}`,
    listModules: () => Promise.resolve([]),
    openEvents: () => () => {},
    ...overrides,
  };
}

function wrap(value: ViewerHostValue, id = 'tv.kroma.probe') {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={new QueryClient()}>
      <ViewerHostProvider value={value}>
        <ModuleScope id={id}>{children}</ModuleScope>
      </ViewerHostProvider>
    </QueryClientProvider>
  );
}

describe('useViewerApi', () => {
  it('binds to the id of the module the host is rendering and no other', () => {
    const moduleApi = vi.fn((id: ModuleId) => ({ id }) as never);
    const { result } = renderHook(() => useViewerApi(), {
      wrapper: wrap(host({ moduleApi })),
    });

    expect(moduleApi).toHaveBeenCalledWith('tv.kroma.probe');
    expect(result.current).toEqual({ id: 'tv.kroma.probe' });
  });

  it('keeps the same client across renders', () => {
    const { result, rerender } = renderHook(() => useViewerApi(), { wrapper: wrap(host()) });
    const first = result.current;
    rerender();
    expect(result.current).toBe(first);
  });

  it('throws outside a module page', () => {
    const quiet = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() =>
      renderHook(() => useViewerApi(), {
        wrapper: ({ children }) => (
          <ViewerHostProvider value={host()}>{children}</ViewerHostProvider>
        ),
      }),
    ).toThrow(/inside a module page/);
    quiet.mockRestore();
  });

  it('throws when the shell mounts no viewer host', () => {
    const quiet = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() =>
      renderHook(() => useViewerApi(), {
        wrapper: ({ children }) => <ModuleScope id="tv.kroma.probe">{children}</ModuleScope>,
      }),
    ).toThrow(/ViewerHostProvider/);
    quiet.mockRestore();
  });
});

describe('useModuleFileUrl', () => {
  it("resolves a path against the module's own id", () => {
    const { result } = renderHook(() => useModuleFileUrl(), { wrapper: wrap(host()) });
    expect(result.current('live/index.m3u8')).toBe('/files/tv.kroma.probe/live/index.m3u8');
  });

  it("builds the url of another module's published file when given its id", () => {
    const { result } = renderHook(() => useModuleFileUrl(), { wrapper: wrap(host()) });
    expect(result.current('clips/g1.mp4', 'tv.kroma.live')).toBe(
      '/files/tv.kroma.live/clips/g1.mp4',
    );
  });
});

describe('useServerEvents outside the admin shell', () => {
  function Listener({ seen }: Readonly<{ seen: string[] }>) {
    useServerEvents<{ type: string }>((e) => seen.push(e.type));
    return <span>listening</span>;
  }

  it('opens one socket through the viewer host and forwards the latest handler', () => {
    let handlers: ViewerEventHandlers | undefined;
    const close = vi.fn();
    const openEvents = vi.fn((h: ViewerEventHandlers) => {
      handlers = h;
      return close;
    });
    const seen: string[] = [];

    const { unmount } = render(<Listener seen={seen} />, {
      wrapper: wrap(host({ openEvents })),
    });
    act(() => handlers?.onEvent({ type: 'party.message' }));

    expect(openEvents).toHaveBeenCalledTimes(1);
    expect(seen).toEqual(['party.message']);
    expect(screen.getByText('listening')).toBeTruthy();
    unmount();
    expect(close).toHaveBeenCalledTimes(1);
  });

  it('reports every reconnect to onOpen', () => {
    let handlers: ViewerEventHandlers | undefined;
    const onOpen = vi.fn();
    function Opener() {
      useServerEvents(() => {}, onOpen);
      return null;
    }
    render(<Opener />, {
      wrapper: wrap(
        host({
          openEvents: (h) => {
            handlers = h;
            return () => {};
          },
        }),
      ),
    });

    act(() => handlers?.onOpen?.());
    act(() => handlers?.onOpen?.());

    expect(onOpen).toHaveBeenCalledTimes(2);
  });

  it('throws with neither shell mounted', () => {
    const quiet = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => render(<Listener seen={[]} />)).toThrow(/ViewerHostProvider/);
    quiet.mockRestore();
  });
});
