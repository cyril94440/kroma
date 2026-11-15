// @vitest-environment jsdom
import { KromaApiError } from '@kromatv/client';
import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const toast = vi.fn();
const go = vi.fn();
vi.mock('@kromatv/ui/kit', () => ({ toast }));
vi.mock('@kromatv/ui', () => ({
  useT: () => (key: string, vars?: object) => `${key} ${JSON.stringify(vars ?? {})}`,
}));
vi.mock('#tv/app/router', () => ({
  useNav: () => ({ go }),
  useClient: () => ({ sport: { clipUrl: () => 'http://s/clip.mp4' } }),
}));

const { useLiveHandlers } = await import('#tv/features/live/useLiveHandlers');

const actions = (over: Record<string, unknown> = {}) =>
  ({
    zap: vi.fn(async () => 'voteStarted'),
    zapByName: vi.fn(async () => 'switchedDirectly'),
    chat: vi.fn(async () => ({})),
    react: vi.fn(async () => ({})),
    pronoVote: vi.fn(async () => ({})),
    watchSport: vi.fn(async () => ({})),
    ...over,
  }) as never;

const handlers = (over?: Record<string, unknown>) =>
  renderHook(() => useLiveHandlers(actions(over), vi.fn())).result.current;
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
const lastMessage = () => toast.mock.calls.at(-1)?.[0].message;

beforeEach(() => {
  toast.mockClear();
  go.mockClear();
});

describe('the live handlers', () => {
  it('confirms a quick reply that went through', async () => {
    const chat = vi.fn(async () => ({}));
    handlers({ chat }).reply('Great game');
    await settle();

    expect([chat.mock.calls, lastMessage()]).toEqual([[['Great game']], 'live.sent {}']);
  });

  it('says when a quick reply did not go through', async () => {
    handlers({ chat: vi.fn(async () => Promise.reject(new Error('down'))) }).reply('Yes');
    await settle();

    expect(lastMessage()).toBe('live.sendFailed {}');
  });

  it('tells the refusal of a closed prono in words', async () => {
    const refused = new KromaApiError(409, 'x', { error: 'noOpenProno' });
    handlers({ pronoVote: vi.fn(async () => Promise.reject(refused)) }).voteProno('p1', '1-0');
    await settle();

    expect(lastMessage()).toBe('live.prono.error.noOpenProno {}');
  });

  it('opens the clip screen with the clip file and its caption', () => {
    handlers().playClip({
      minute: "37'",
      buteur: 'Kebbal',
      dom: 'Rennes',
      ext: 'OM',
      score: '1-0',
    } as never);

    expect(go).toHaveBeenCalledWith('clip', {
      url: 'http://s/clip.mp4',
      title: "37' Kebbal",
      subtitle: 'Rennes 1-0 OM',
    });
  });

  it('refuses a sport row with no channel instead of zapping', async () => {
    const watchSport = vi.fn();
    handlers({ watchSport }).watchSport({ chaine: null } as never);
    await settle();

    expect([watchSport.mock.calls.length, lastMessage()]).toEqual([0, 'live.sport.noChannel {}']);
  });

  it('speaks the outcome of a zap from a card channel name', async () => {
    handlers().zapByName('Canal+ Foot');
    await settle();

    expect(lastMessage()).toBe('live.zapDirect {"name":"Canal+ Foot"}');
  });
});
