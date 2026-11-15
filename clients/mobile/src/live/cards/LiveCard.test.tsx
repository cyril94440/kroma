// @vitest-environment jsdom
import { onScreen } from '@kromatv/ui/testing';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('#mobile/lib/i18n', async () => {
  const { english } = await import('#mobile/live/i18n.fixture');
  return { useT: () => english };
});

const { LiveCard } = await import('#mobile/live/cards/LiveCard');

afterEach(cleanup);

const show = (card: unknown, onWatch = vi.fn()) => {
  render(onScreen(<LiveCard card={card} onWatch={onWatch} />));
  return onWatch;
};

describe('a Hermès card on the phone', () => {
  it('draws a table with its rows and note', () => {
    show({
      kind: 'table',
      title: 'Classement',
      columns: ['#', 'Club'],
      rows: [['1', 'PSG']],
      note: 'après la 11e journée',
    });

    expect(
      ['Classement', 'PSG', 'après la 11e journée'].map((t) => screen.queryByText(t) !== null),
    ).toEqual([true, true, true]);
  });

  it('asks to watch the channel of a match card', () => {
    const onWatch = show({
      kind: 'match',
      home: 'PSG',
      away: 'OM',
      channel: 'Canal+',
      action: { id: 'watch', label: 'Put this channel on' },
    });

    fireEvent.click(screen.getByRole('button', { name: 'Put this channel on' }));

    expect(onWatch).toHaveBeenCalledWith('Canal+');
  });

  it('names a chart for assistive tech by its first and last value', () => {
    show({
      kind: 'chart',
      title: 'Débit',
      style: 'line',
      unit: 'kb/s',
      points: [
        { label: '20:00', value: 3800 },
        { label: '20:10', value: 3650 },
      ],
    });

    expect(screen.getByLabelText('Débit: 3800 kb/s to 3650 kb/s')).toBeTruthy();
  });

  it('draws nothing for a card outside the catalogue', () => {
    show({ kind: 'html', body: '<b>x</b>' });

    expect(document.body.textContent).toBe('');
  });
});
