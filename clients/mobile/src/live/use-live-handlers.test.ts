// @vitest-environment jsdom
import { KromaApiError } from '@kromatv/client';
import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const toast = vi.fn();
const push = vi.fn();
vi.mock('@kromatv/ui/kit', () => ({ toast }));
vi.mock('expo-router', () => ({ useRouter: () => ({ push }) }));
vi.mock('#mobile/lib/i18n', () => ({
  useT: () => (key: string, vars?: object) => `${key} ${JSON.stringify(vars ?? {})}`,
}));
vi.mock('#mobile/lib/session', () => ({
  useClient: () => ({ sport: { clipUrl: () => 'http://s/clip.mp4' } }),
}));

const { useLiveHandlers } = await import('./use-live-handlers');

const actions = (over: Record<string, unknown> = {}) =>
  ({
    zapByName: vi.fn(async () => 'voteStarted'),
    chat: vi.fn(async () => ({})),
    react: vi.fn(async () => ({})),
    pronoVote: vi.fn(async () => ({})),
    watchSport: vi.fn(async () => ({})),
    ...over,
  }) as never;

const handlers = (over?: Record<string, unknown>) =>
  renderHook(() => useLiveHandlers(actions(over))).result.current;
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
const lastMessage = () => toast.mock.calls.at(-1)?.[0].message;

beforeEach(() => {
  toast.mockClear();
  push.mockClear();
});

describe('the phone live handlers', () => {
  it('says when a message did not go through', async () => {
    handlers({ chat: vi.fn(async () => Promise.reject(new Error('down'))) }).reply('Yes');
    await settle();

    expect(lastMessage()).toBe('live.sendFailed {}');
  });

  it('posts a message without a toast when it goes through', async () => {
    const chat = vi.fn(async () => ({}));
    handlers({ chat }).reply('Yes');
    await settle();

    expect([chat.mock.calls, toast.mock.calls.length]).toEqual([[['Yes']], 0]);
  });

  it('tells a closed prono in words', async () => {
    const refused = new KromaApiError(409, 'x', { error: 'noOpenProno' });
    handlers({ pronoVote: vi.fn(async () => Promise.reject(refused)) }).voteProno('p1', '1-0');
    await settle();

    expect(lastMessage()).toBe('live.prono.error.noOpenProno {}');
  });

  it('opens the clip route with the file and its caption', () => {
    handlers().playClip({
      minute: "37'",
      buteur: 'Kebbal',
      dom: 'Rennes',
      ext: 'OM',
      score: '1-0',
    } as never);

    const href = String(push.mock.calls[0]?.[0]);
    const query = new URLSearchParams(href.split('?')[1]);
    expect([
      href.startsWith('/clip?'),
      query.get('uri'),
      query.get('title'),
      query.get('subtitle'),
    ]).toEqual([true, 'http://s/clip.mp4', "37' Kebbal", 'Rennes 1-0 OM']);
  });

  it('refuses a sport row without a channel', async () => {
    const watchSport = vi.fn();
    handlers({ watchSport }).watchSport({ chaine: null } as never);
    await settle();

    expect([watchSport.mock.calls.length, lastMessage()]).toEqual([0, 'live.sport.noChannel {}']);
  });
});
