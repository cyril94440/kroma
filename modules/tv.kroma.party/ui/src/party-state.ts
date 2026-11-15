import type { ChatMessage, PartyEvent, PartyState, VoteClosed } from './schemas';

export const HISTORY_MAX = 100;
const FLOATING_MAX = 12;

export interface Floating {
  key: string;
  name: string;
  emoji: string;
  at: number;
}

export interface Model {
  state: PartyState | null;
  seq: number;
  stale: boolean;
  floating: Floating[];
  lastClosed: (VoteClosed & { at: number }) | null;
  kicked: string | null;
  kickedAt: number;
  goLiveAt: number;
  fineSyncAt: number;
  ideasSeq: number;
}

export const EMPTY: Model = {
  state: null,
  seq: 0,
  stale: false,
  floating: [],
  lastClosed: null,
  kicked: null,
  kickedAt: 0,
  goLiveAt: 0,
  fineSyncAt: 0,
  ideasSeq: 0,
};

export function adopt(model: Model, state: PartyState): Model {
  return { ...model, state, seq: state.seq, stale: false };
}

function withMessage(messages: ChatMessage[], m: ChatMessage): ChatMessage[] {
  if (messages.some((x) => x.id === m.id)) return messages;
  return [...messages, m].slice(-HISTORY_MAX);
}

function patch(state: PartyState, event: PartyEvent): PartyState {
  switch (event.type) {
    case 'party.message': {
      const { type: _type, seq: _seq, ...message } = event;
      return { ...state, messages: withMessage(state.messages, message) };
    }
    case 'party.reactions':
      return {
        ...state,
        messages: state.messages.map((m) =>
          m.id === event.id ? { ...m, reactions: event.reactions } : m,
        ),
      };
    case 'party.vote':
      return { ...state, vote: event.vote };
    case 'party.poll':
      return { ...state, poll: event.poll, lastPoll: event.closed ?? state.lastPoll };
    case 'party.activity':
      return { ...state, activity: event.label };
    case 'party.sync':
      return { ...state, sync: event.state.members.length ? event.state : null };
    default:
      return state;
  }
}

/** Folds one live event into the model. An event whose sequence is at or below
 *  the one the snapshot carried is already in it and is dropped; one that skips
 *  ahead marks the model stale so the caller refetches the state. Addressed
 *  events carry no sequence and always apply. */
export function applyEvent(model: Model, event: PartyEvent, now: number): Model {
  if (!model.state) return model;
  if (event.type === 'party.sync.kicked') return { ...model, kicked: event.reason, kickedAt: now };
  if (event.seq <= model.seq) return model;
  const gap = model.seq > 0 && event.seq > model.seq + 1;
  const base: Model = { ...model, seq: event.seq, stale: model.stale || gap };
  switch (event.type) {
    case 'party.reaction':
      return {
        ...base,
        floating: [
          ...base.floating,
          { key: `${event.seq}`, name: event.name, emoji: event.emoji, at: now },
        ].slice(-FLOATING_MAX),
      };
    case 'party.vote':
      return {
        ...base,
        state: patch(model.state, event),
        lastClosed: event.closed ? { ...event.closed, at: now } : base.lastClosed,
      };
    case 'party.sync.golive':
      return { ...base, goLiveAt: now };
    case 'party.sync.finesync':
      return { ...base, fineSyncAt: now };
    case 'party.ideas':
      return { ...base, ideasSeq: event.seq };
    default:
      return { ...base, state: patch(model.state, event) };
  }
}

export function expireFloating(model: Model, now: number, ttlMs: number): Model {
  const kept = model.floating.filter((f) => now - f.at < ttlMs);
  return kept.length === model.floating.length ? model : { ...model, floating: kept };
}
