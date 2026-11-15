// @vitest-environment jsdom
import { createInputTestHost, onScreen } from '@kromatv/ui/testing';
import { cleanup, fireEvent, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ReactionsDrawer } from '#tv/features/live/ReactionsDrawer';

const { clearPressGuard, render } = createInputTestHost();

afterEach(() => {
  cleanup();
  clearPressGuard();
});

describe('the reactions drawer', () => {
  it('names each reaction in words and sends the glyph on press', async () => {
    const onReact = vi.fn();
    render(onScreen(<ReactionsDrawer open onClose={vi.fn()} onReact={onReact} />));

    const goal = await screen.findByRole('button', { name: 'Goal' });
    clearPressGuard();
    fireEvent.click(goal);
    void (await screen.findByRole('button', { name: 'Goal' }));

    expect(onReact).toHaveBeenCalledWith('⚽');
    expect(screen.getAllByRole('button').map((b) => b.getAttribute('aria-label'))).toEqual(
      expect.arrayContaining(['Goal', 'On fire', 'Laughing', 'Surprised', 'Love', 'Applause']),
    );
  });
});
