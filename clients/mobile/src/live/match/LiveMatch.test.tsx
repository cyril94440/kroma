// @vitest-environment jsdom
import { onScreen } from '@kromatv/ui/testing';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('#mobile/lib/i18n', async () => {
  const { english } = await import('#mobile/live/i18n.fixture');
  return { useT: () => english };
});

const { LiveMatch } = await import('#mobile/live/match/LiveMatch');
const { GOAL_AT, KICKOFF, liveMatch } = await import('#mobile/live/match/match.fixture');

afterEach(cleanup);

const show = (frame: number | null) =>
  render(
    onScreen(
      <LiveMatch
        data={{ match: liveMatch(), prono: null, clips: [], me: 'u1', postSeenAt: null }}
        frameTimeMs={() => frame}
        now={KICKOFF + 3_600_000}
        onVote={vi.fn()}
        onPlayClip={vi.fn()}
      />,
    ),
  );

describe('the phone match panel', () => {
  it('holds the score back until the frame with the goal has played', () => {
    show(GOAL_AT - 1);

    expect(screen.getByText('0 - 0')).toBeTruthy();
  });

  it('shows the goal and its scorer once the picture reaches it', () => {
    show(GOAL_AT);

    expect([
      screen.queryByText('1 - 0') !== null,
      screen.queryByText('Goal Kebbal') !== null,
    ]).toEqual([true, true]);
  });

  it('draws the line-ups and the stats from their tabs', () => {
    show(GOAL_AT);

    fireEvent.click(screen.getByRole('radio', { name: 'Line-ups' }));
    expect(screen.getByText('Mandanda')).toBeTruthy();
    fireEvent.click(screen.getByRole('radio', { name: 'Stats' }));

    expect(screen.getByText('Momentum')).toBeTruthy();
  });
});
