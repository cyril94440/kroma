// @vitest-environment jsdom
import { onScreen } from '@kromatv/ui/testing';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CardView } from '#tv/features/live/cards/CardView';

afterEach(cleanup);

const table = {
  kind: 'table',
  title: 'Classement Ligue 1',
  columns: ['#', 'Club', 'Pts'],
  rows: [
    ['1', 'PSG', 28],
    ['2', 'Lens', 25],
  ],
  note: 'après la 11e journée',
};
const match = {
  kind: 'match',
  home: 'PSG',
  away: 'Liverpool',
  competition: 'Ligue des champions',
  score: '1 - 0',
  status: 'live',
  channel: 'Canal+ Foot',
  action: { id: 'watch', label: 'Mettre cette chaîne' },
};

const show = (card: unknown, onWatch = vi.fn()) => {
  render(onScreen(<CardView card={card} onWatch={onWatch} />));
  return onWatch;
};

describe('a Hermès card', () => {
  it('draws a table with its title, rows and note', () => {
    show(table);

    expect(
      ['Classement Ligue 1', 'PSG', '28', 'Lens', 'après la 11e journée'].map(
        (text) => screen.queryByText(text) !== null,
      ),
    ).toEqual([true, true, true, true, true]);
  });

  it('draws a match with its score and asks to watch its channel', () => {
    const onWatch = show(match);

    fireEvent.click(screen.getByLabelText('Mettre cette chaîne'));

    expect([screen.getByText('1 - 0') !== null, onWatch.mock.calls]).toEqual([
      true,
      [['Canal+ Foot']],
    ]);
  });

  it('draws nothing for a card outside the catalogue', () => {
    show({ kind: 'html', body: '<b>x</b>' });

    expect(screen.queryByText('x')).toBeNull();
  });

  it('shows markup in a cell as plain text', () => {
    show({ ...table, rows: [['1', '<img src=x onerror=alert(1)>', 3]] });

    expect(screen.getByText('<img src=x onerror=alert(1)>')).toBeTruthy();
    expect(document.querySelector('img')).toBeNull();
  });
});
