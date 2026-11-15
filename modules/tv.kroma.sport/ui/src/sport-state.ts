import type { SportEventMessage, SportState } from './schemas';

export interface Model {
  state: SportState | null;
  seq: number;
  stale: boolean;
  clockOffsetMs: number;
  postSeen: { id: string; at: number } | null;
}

export const EMPTY: Model = { state: null, seq: 0, stale: false, clockOffsetMs: 0, postSeen: null };

function trackPost(
  previous: Model['postSeen'],
  match: SportState['match'],
  serverNow: number,
): Model['postSeen'] {
  if (match?.statut !== 'post') return null;
  return previous?.id === match.id ? previous : { id: match.id, at: serverNow };
}

export function adopt(state: SportState, receivedAt: number): Model {
  const clockOffsetMs = state.now - receivedAt;
  return {
    state,
    seq: state.seq,
    stale: false,
    clockOffsetMs,
    postSeen: trackPost(null, state.match, state.now),
  };
}

function patch(state: SportState, event: SportEventMessage): SportState {
  switch (event.type) {
    case 'sport.view':
      return { ...state, view: event.view };
    case 'sport.match':
      return { ...state, match: event.match };
    case 'sport.prono':
      return { ...state, prono: event.prono };
    case 'sport.clips':
      return { ...state, clips: event.clips };
  }
}

/** Folds one live event into the model: an event the snapshot already holds is dropped, one that skips ahead marks the model stale so the caller refetches. */
export function applyEvent(model: Model, event: SportEventMessage, receivedAt: number): Model {
  if (!model.state || event.seq <= model.seq) return model;
  const gap = model.seq > 0 && event.seq > model.seq + 1;
  const state = patch(model.state, event);
  const postSeen = trackPost(model.postSeen, state.match, receivedAt + model.clockOffsetMs);
  return { ...model, state, seq: event.seq, stale: model.stale || gap, postSeen };
}
