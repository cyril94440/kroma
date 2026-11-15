// @vitest-environment jsdom
import type { PronoPayload } from '@kromatv/client/sport';
import { createInputTestHost, onScreen } from '@kromatv/ui/testing';
import { cleanup, fireEvent, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PronoTab } from '#tv/features/live/match/PronoTab';

const { clearPressGuard, render } = createInputTestHost();

afterEach(() => {
  cleanup();
  clearPressGuard();
});

const prono = (extra: Partial<PronoPayload> = {}): PronoPayload => ({
  id: 'p1',
  competition: 'Ligue 1',
  home: 'Rennes',
  away: 'Marseille',
  home_abbr: 'REN',
  away_abbr: 'OM',
  home_color: '#c00',
  away_color: '#09f',
  statut: 'ouvert',
  chaine: '',
  score: '0-0',
  minute: '',
  auto: true,
  ferme_dans_s: 40,
  votes: {},
  noms: {},
  requis: ['u1', 'u2'],
  distribution: [],
  classement_match: [],
  resultats: [],
  saison: [],
  ...extra,
});

const show = (value: PronoPayload | null, me = 'u1') => {
  const onVote = vi.fn();
  render(onScreen(<PronoTab prono={value} me={me} onVote={onVote} />));
  return onVote;
};

const press = async (name: string) => {
  const button = await screen.findByRole('button', { name });
  clearPressGuard();
  fireEvent.click(button);
};

describe('the prono tab', () => {
  it('says when no prediction is open', () => {
    show(null);

    expect(screen.getByText(/No prediction open/)).toBeTruthy();
  });

  it('sends the score the viewer built with the steppers', async () => {
    const onVote = show(prono());

    await press('One more goal for REN');
    await press('One more goal for REN');
    await press('One more goal for OM');
    await press('Validate');

    expect(onVote).toHaveBeenCalledWith('p1', '2-1');
  });

  it('never goes below zero', async () => {
    const onVote = show(prono());

    await press('One goal fewer for REN');
    await press('Validate');

    expect(onVote).toHaveBeenCalledWith('p1', '0-0');
  });

  it('shows the viewer their own prediction while it is open', () => {
    show(prono({ votes: { u1: '1-0' } }));

    expect(screen.getByText('Your prediction: 1-0')).toBeTruthy();
  });

  it('shows the results with points once the match is final', () => {
    show(
      prono({
        statut: 'final',
        score: '2-1',
        resultats: [{ user_id: 'u1', pseudo: 'Ana', prono: '2-1', points: 3, exact: true }],
      }),
    );

    expect([screen.getByText('Ana 2-1') !== null, screen.getByText('+3') !== null]).toEqual([true, true]);
  });

  it('says a cancelled prediction is cancelled', () => {
    show(prono({ annule: true, statut: 'en_cours' }));

    expect(screen.getByText(/Prediction cancelled/)).toBeTruthy();
  });
});
