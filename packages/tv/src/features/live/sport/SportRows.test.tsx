// @vitest-environment jsdom
import { SportEvent } from '@kromatv/client/sport';
import { createInputTestHost, onScreen } from '@kromatv/ui/testing';
import { cleanup, fireEvent, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SportRows } from '#tv/features/live/sport/SportRows';

const { clearPressGuard, render } = createInputTestHost();

afterEach(() => {
  cleanup();
  clearPressGuard();
});

const event = (id: string, extra: object = {}) =>
  SportEvent.parse({
    id,
    sport: 'foot',
    statut: 'in',
    debut: 1_800_000_000_000,
    temps: "37'",
    lignes: [
      { nom: 'Rennes', score: '1' },
      { nom: 'OM', score: '0' },
    ],
    chaine: { nom: 'Ligue 1+ 1', ref: 'p:12' },
    ...extra,
  });

describe('the sport rows', () => {
  it('lists what is on air with its score and channel, and puts it on when pressed', async () => {
    const onWatch = vi.fn();
    const live = event('e1');
    render(onScreen(<SportRows rows={{ live: [live], upcoming: [] }} onWatch={onWatch} />));
    const row = await screen.findByText('Rennes 1 - 0 OM');
    clearPressGuard();

    fireEvent.click(row);

    expect([screen.getByText("37' · Ligue 1+ 1") !== null, onWatch.mock.calls]).toEqual([
      true,
      [[live]],
    ]);
  });

  it('hides the score of a match the picture would spoil', () => {
    const hidden = event('e2', { masquer_score: true });
    render(onScreen(<SportRows rows={{ live: [hidden], upcoming: [] }} onWatch={vi.fn()} />));

    expect(screen.getByText('Rennes vs OM')).toBeTruthy();
  });

  it('says when nothing is scheduled', () => {
    render(onScreen(<SportRows rows={{ live: [], upcoming: [] }} onWatch={vi.fn()} />));

    expect(screen.getByText('Nothing scheduled right now')).toBeTruthy();
  });
});
