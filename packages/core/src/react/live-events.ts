import type { KromaClient } from '@kromatv/client';
import { KromaEvents } from '@kromatv/client/events';

export interface EventHandlers {
  onEvent(event: { type: string }): void;
  onOpen(): void;
}

export type OpenEvents = (handlers: EventHandlers) => () => void;

/** One socket on `/api/events` for a whole live screen, closed with it. */
export function socketEvents(client: KromaClient): OpenEvents {
  return ({ onEvent, onOpen }) => {
    const events = new KromaEvents<{ type: string }>(client.baseUrl, {
      token: () => client.sessionToken,
      refresh: () => client.refreshSession(),
      onEvent,
      onOpen,
    });
    events.connect();
    return () => events.close();
  };
}
