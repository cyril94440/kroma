import { describe, expect, it } from 'vitest';
import { chartBars, parseCard, rowTone } from './cards';

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
const chart = {
  kind: 'chart',
  title: 'Débit du flux (1 h)',
  style: 'line',
  unit: 'kb/s',
  points: [
    { label: '20:00', value: 3800 },
    { label: '20:10', value: 3650 },
  ],
};
const match = {
  kind: 'match',
  home: 'PSG',
  away: 'Liverpool',
  competition: 'Ligue des champions',
  start: 1_793_300_400,
  score: '1 - 0',
  status: 'live',
  channel: 'Canal+ Foot',
  action: { id: 'watch', label: 'Mettre cette chaîne' },
};

describe('parseCard', () => {
  it.each([table, chart, match])('reads the catalogue card $kind', (card) => {
    expect(parseCard(card)?.kind).toBe(card.kind);
  });

  it.each([null, 'table', [], {}, { kind: 'html' }, { kind: 'Table' }, { ...match, action: { id: 'x', label: 'y' } }])(
    'ignores %j',
    (card) => {
      expect(parseCard(card)).toBeNull();
    },
  );
});

describe('chartBars', () => {
  it('scales every point against the largest', () => {
    expect(chartBars(chart.points).map((b) => b.ratio)).toEqual([1, 3650 / 3800]);
  });

  it('draws nothing tall when every value is zero', () => {
    expect(chartBars([{ label: 'a', value: 0 }]).map((b) => b.ratio)).toEqual([0]);
  });
});

describe('rowTone', () => {
  const live = { rows: [{ live: true, outcome: 'win' }, { live: false, outcome: null }, { live: true, outcome: null }] };

  it('marks a live row by its outcome and leaves a settled one alone', () => {
    expect([rowTone({ live }, 0), rowTone({ live }, 1), rowTone({ live }, 2), rowTone({}, 0)]).toEqual(['win', null, 'live', null]);
  });
});
