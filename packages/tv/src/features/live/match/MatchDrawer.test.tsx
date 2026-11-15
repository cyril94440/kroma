// @vitest-environment jsdom
import { createInputTestHost, onScreen } from '@kromatv/ui/testing';
import { cleanup, fireEvent, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MatchDrawer } from '#tv/features/live/match/MatchDrawer';
import { GOAL_AT, KICKOFF, liveMatch } from '#tv/features/live/match/match.fixture';

const { clearPressGuard, render } = createInputTestHost();

afterEach(() => {
  cleanup();
  clearPressGuard();
});

const show = (
  frame: number | null,
  data: Partial<Parameters<typeof MatchDrawer>[0]['data']> = {},
) =>
  render(
    onScreen(
      <MatchDrawer
        open
        data={{ match: liveMatch(), prono: null, clips: [], me: 'u1', postSeenAt: null, ...data }}
        frameTimeMs={() => frame}
        now={KICKOFF + 3_600_000}
        onClose={vi.fn()}
        onVote={vi.fn()}
        onPlayClip={vi.fn()}
      />,
    ),
  );

const tab = async (name: string) => {
  const segment = await screen.findByRole('button', { name });
  clearPressGuard();
  fireEvent.click(segment);
};

describe('the match drawer', () => {
  it('holds the score back until the frame with the goal has played', () => {
    show(GOAL_AT - 1);

    expect(screen.getByText('0 - 0')).toBeTruthy();
  });

  it('shows the goal once the picture reaches it', () => {
    show(GOAL_AT);

    expect(screen.getByText('1 - 0')).toBeTruthy();
    expect(screen.getByText('Goal Kebbal')).toBeTruthy();
  });

  it('draws the line-ups on a pitch from the Line-ups tab', async () => {
    show(GOAL_AT);

    await tab('Line-ups');

    expect(screen.getByText('Mandanda')).toBeTruthy();
  });

  it('shows the counters and the momentum from the Stats tab', async () => {
    show(GOAL_AT);

    await tab('Stats');

    expect([screen.getByText('Shots') !== null, screen.getByText('Momentum') !== null]).toEqual([
      true,
      true,
    ]);
  });

  it('lists a clip only once the picture has passed the goal', async () => {
    const clip = {
      id: 'c1',
      match_id: 'm1',
      dom: 'Rennes',
      ext: 'OM',
      competition: '',
      minute: "10'",
      equipe: 'dom',
      buteur: 'Kebbal',
      score: '1-0',
      debut_ms: GOAL_AT - 8000,
      fin_ms: GOAL_AT + 4000,
      duree_s: 12,
      ts: 0,
      file: { module: 'tv.kroma.live', path: 'clips/c1.mp4' },
    };
    show(GOAL_AT, { clips: [clip] });
    await tab('Goal clips');
    expect(screen.queryByText("10' Kebbal")).toBeNull();

    cleanup();
    show(GOAL_AT + 4000, { clips: [clip] });
    await tab('Goal clips');

    expect(screen.getByText("10' Kebbal")).toBeTruthy();
  });
});
