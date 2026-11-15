import type { SportEventMessage, SportState } from '@kromatv/client/sport';

export interface SportModel {
  state: SportState | null;
  seq: number;
  stale: boolean;
  clockOffsetMs: number;
  postSeen: { id: string; at: number } | null;
}

export const EMPTY_SPORT: SportModel = {
  state: null,
  seq: 0,
  stale: false,
  clockOffsetMs: 0,
  postSeen: null,
};

function trackPost(
  previous: SportModel['postSeen'],
  match: SportState['match'],
  serverNow: number,
): SportModel['postSeen'] {
  if (match?.statut !== 'post') return null;
  return previous?.id === match.id ? previous : { id: match.id, at: serverNow };
}

export function adoptSport(state: SportState, receivedAt: number): SportModel {
  return {
    state,
    seq: state.seq,
    stale: false,
    clockOffsetMs: state.now - receivedAt,
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

/** An event the snapshot already holds is dropped, one that skips ahead marks
 *  the model stale so the caller refetches. `postSeen` remembers when this
 *  viewer first saw the match finished, which the clock waits the delay on. */
export function applySportEvent(
  model: SportModel,
  event: SportEventMessage,
  receivedAt: number,
): SportModel {
  if (!model.state || event.seq <= model.seq) return model;
  const gap = model.seq > 0 && event.seq > model.seq + 1;
  const state = patch(model.state, event);
  const postSeen = trackPost(model.postSeen, state.match, receivedAt + model.clockOffsetMs);
  return { ...model, state, seq: event.seq, stale: model.stale || gap, postSeen };
}
