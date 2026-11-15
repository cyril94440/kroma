import type { SportEventMessage, SportState } from '@kromatv/client/sport';

export interface SportModel {
  state: SportState | null;
  seq: number;
  stale: boolean;
  clockOffsetMs: number;
}

export const EMPTY_SPORT: SportModel = { state: null, seq: 0, stale: false, clockOffsetMs: 0 };

export function adoptSport(state: SportState, receivedAt: number): SportModel {
  return { state, seq: state.seq, stale: false, clockOffsetMs: state.now - receivedAt };
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
 *  the model stale so the caller refetches. */
export function applySportEvent(model: SportModel, event: SportEventMessage): SportModel {
  if (!model.state || event.seq <= model.seq) return model;
  const gap = model.seq > 0 && event.seq > model.seq + 1;
  return { ...model, state: patch(model.state, event), seq: event.seq, stale: model.stale || gap };
}
