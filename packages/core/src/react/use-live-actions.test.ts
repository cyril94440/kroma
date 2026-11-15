// @vitest-environment jsdom
import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useLiveActions } from './use-live-actions';
import type { LiveClient } from './use-live-room';

function setup(outcome: 'voteStarted' | 'switchedDirectly' | 'refused' = 'voteStarted') {
  const requestSwitch = vi.fn(async () => ({ outcome }));
  const switchTo = vi.fn(async () => ({ status: 'switched' }));
  const client = { party: { requestSwitch }, live: { switchTo } } as unknown as LiveClient;
  return { client, requestSwitch, switchTo };
}

const run = (client: LiveClient, party: boolean) =>
  renderHook(() => useLiveActions(client, { live: true, party, sport: false })).result.current;

describe('the live screen actions', () => {
  it('asks the party to decide a zap when it is installed', async () => {
    const { client, requestSwitch, switchTo } = setup('voteStarted');

    const outcome = await run(client, true).zap({ id: 'p:1', name: 'TF1' });

    expect(outcome).toBe('voteStarted');
    expect(requestSwitch).toHaveBeenCalledWith('p:1', 'TF1');
    expect(switchTo).not.toHaveBeenCalled();
  });

  it('has the live module switch at once when there is no party', async () => {
    const { client, requestSwitch, switchTo } = setup();

    const outcome = await run(client, false).zap({ id: 'p:2', name: 'M6' });

    expect(outcome).toBe('switchedDirectly');
    expect(switchTo).toHaveBeenCalledWith('p:2');
    expect(requestSwitch).not.toHaveBeenCalled();
  });

  it('passes a refusal through', async () => {
    const { client } = setup('refused');

    expect(await run(client, true).zap({ id: 'p:3', name: 'Arte' })).toBe('refused');
  });

  it('resolves a card channel name to a channel and zaps to it', async () => {
    const requestSwitch = vi.fn(async () => ({ outcome: 'voteStarted' as const }));
    const channels = vi.fn(async () => ({ channels: [{ id: 'p:9', name: 'Canal+ Foot', group: '' }] }));
    const client = { party: { requestSwitch, channels } } as unknown as LiveClient;

    const outcome = await run(client, true).zapByName('Canal+ Foot');

    expect([outcome, channels.mock.calls[0], requestSwitch.mock.calls[0]]).toEqual([
      'voteStarted',
      ['Canal+ Foot'],
      ['p:9', 'Canal+ Foot'],
    ]);
  });

  it('refuses a card channel nobody carries', async () => {
    const channels = vi.fn(async () => ({ channels: [] }));
    const client = { party: { channels } } as unknown as LiveClient;

    expect(await run(client, true).zapByName('Nowhere')).toBe('refused');
  });

  it('sends a prono, a sport watch and an idea to their modules', async () => {
    const vote = vi.fn(async () => ({ ok: true }));
    const watch = vi.fn(async () => ({ outcome: 'ok' }));
    const submitIdea = vi.fn(async () => ({ id: 'i1' }));
    const client = { sport: { vote, watch }, party: { submitIdea } } as unknown as LiveClient;
    const actions = run(client, true);

    await actions.pronoVote('m1', '2-1');
    await actions.watchSport('p:1');
    await actions.idea('bug', 'no sound');

    expect([vote.mock.calls[0], watch.mock.calls[0], submitIdea.mock.calls[0]]).toEqual([
      ['m1', '2-1'],
      ['p:1'],
      ['bug', 'no sound'],
    ]);
  });
});
