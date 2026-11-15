// @vitest-environment jsdom
import type { PronoPayload } from '@kromatv/client/sport';
import { onScreen } from '@kromatv/ui/testing';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('#mobile/lib/i18n', async () => {
  const { english } = await import('#mobile/live/i18n.fixture');
  return { useT: () => english };
});

const { MatchProno } = await import('#mobile/live/match/MatchProno');

afterEach(cleanup);

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

const press = (name: string) => fireEvent.click(screen.getByRole('button', { name }));

describe('the phone prono', () => {
  it('sends the score built with the steppers', () => {
    const onVote = vi.fn();
    render(onScreen(<MatchProno prono={prono()} me="u1" onVote={onVote} />));

    press('One more goal for REN');
    press('One more goal for REN');
    press('One more goal for OM');
    press('Validate');

    expect(onVote).toHaveBeenCalledWith('p1', '2-1');
  });

  it('shows the viewer their own prediction', () => {
    render(
      onScreen(<MatchProno prono={prono({ votes: { u1: '1-0' } })} me="u1" onVote={vi.fn()} />),
    );

    expect(screen.getByText('Your prediction: 1-0')).toBeTruthy();
  });

  it('says when none is open', () => {
    render(onScreen(<MatchProno prono={null} me="u1" onVote={vi.fn()} />));

    expect(screen.getByText(/No prediction open/)).toBeTruthy();
  });

  it('shows points once the match is final', () => {
    const final = prono({
      statut: 'final',
      score: '2-1',
      resultats: [{ user_id: 'u1', pseudo: 'Ana', prono: '2-1', points: 3, exact: true }],
    });
    render(onScreen(<MatchProno prono={final} me="u1" onVote={vi.fn()} />));

    expect(screen.getByText('+3')).toBeTruthy();
  });
});
