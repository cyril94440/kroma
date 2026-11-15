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
});
