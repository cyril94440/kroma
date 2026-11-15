import { ModuleId } from '@kromatv/client/modules';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const H = vi.hoisted(() => ({
  instances: [] as {
    url: string;
    opts: Record<string, unknown>;
    connect: () => void;
    close: () => void;
  }[],
}));

vi.mock('@kromatv/client/events', () => ({
  KromaEvents: class {
    connect = vi.fn();
    close = vi.fn();
    constructor(
      readonly url: string,
      readonly opts: Record<string, unknown>,
    ) {
      H.instances.push(this as never);
    }
  },
}));

import { createViewerHost } from './viewer-host';

function client() {
  return {
    sessionToken: 'tok',
    refreshSession: vi.fn(async () => 'fresh'),
    modules: {
      viewerApi: vi.fn((id: string) => ({ viewerFor: id })),
      fileUrl: vi.fn((id: string, path: string) => `/api/module-files/${id}/${path}?t=x`),
      list: vi.fn(async () => [{ id: 'tv.kroma.live', name: 'Live' }]),
    },
  };
}

beforeEach(() => {
  H.instances.length = 0;
});

describe('createViewerHost', () => {
  it("hands out the client's viewer mount for exactly the id asked", () => {
    const c = client();
    const host = createViewerHost(c as never, 'https://tv');
    const id = ModuleId.parse('tv.kroma.party');

    expect(host.moduleApi(id)).toEqual({ viewerFor: 'tv.kroma.party' });
    expect(c.modules.viewerApi).toHaveBeenCalledWith('tv.kroma.party');
  });

  it('resolves published files through the client so the media ticket rides along', () => {
    const host = createViewerHost(client() as never, 'https://tv');
    expect(host.fileUrl(ModuleId.parse('tv.kroma.live'), 'a/index.m3u8')).toBe(
      '/api/module-files/tv.kroma.live/a/index.m3u8?t=x',
    );
  });

  it('lists modules through the client', async () => {
    const host = createViewerHost(client() as never, 'https://tv');
    await expect(host.listModules()).resolves.toEqual([{ id: 'tv.kroma.live', name: 'Live' }]);
  });

  it('opens an event socket authenticated and refreshed by the same client', async () => {
    const c = client();
    const onEvent = vi.fn();
    const onOpen = vi.fn();

    const close = createViewerHost(c as never, 'https://tv').openEvents({ onEvent, onOpen });

    const [socket] = H.instances;
    expect(socket?.url).toBe('https://tv');
    const opts = socket?.opts as {
      token: () => string;
      refresh: () => Promise<string>;
      onEvent: unknown;
    };
    expect(opts.token()).toBe('tok');
    await expect(opts.refresh()).resolves.toBe('fresh');
    expect(opts.onEvent).toBe(onEvent);
    expect(socket?.connect).toHaveBeenCalled();
    close();
    expect(socket?.close).toHaveBeenCalled();
  });
});
