// @vitest-environment jsdom
import type { Idea } from '@kromatv/client/party';
import { onScreen } from '@kromatv/ui/testing';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { IdeasDrawer } from '#tv/features/live/IdeasDrawer';

afterEach(cleanup);

const idea: Idea = {
  id: 'i1',
  ts: 1,
  genre: 'bug',
  text: 'No sound on the kitchen TV',
  status: 'inProgress',
  reply: 'Looking into it',
  author: { userId: 'u', name: 'Mamie' },
  updatedAt: 2,
};

describe('the ideas drawer', () => {
  it('reads each idea with its author, status and answer', () => {
    render(onScreen(<IdeasDrawer open ideas={[idea]} onClose={vi.fn()} />));

    expect(
      ['Mamie', 'In progress', 'No sound on the kitchen TV', 'Looking into it'].map(
        (text) => screen.queryByText(text) !== null,
      ),
    ).toEqual([true, true, true, true]);
  });

  it('says the box is empty', () => {
    render(onScreen(<IdeasDrawer open ideas={[]} onClose={vi.fn()} />));

    expect(screen.getByText('No idea yet')).toBeTruthy();
  });
});
