import type {
  ChatMessage,
  PartyEvent,
  PartyState,
  PollView,
  VoteClosed,
  VoteView,
} from '@kromatv/client/party';

export const HISTORY_MAX = 100;
const FLOATING_MAX = 12;

export interface Floating {
  key: string;
  name: string;
  emoji: string;
  at: number;
}

export interface PartyModel {
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

export const EMPTY_PARTY: PartyModel = {
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

export function adoptParty(model: PartyModel, state: PartyState): PartyModel {
  return { ...model, state, seq: state.seq, stale: false };
}

function withMessage(messages: ChatMessage[], message: ChatMessage): ChatMessage[] {
  if (messages.some((existing) => existing.id === message.id)) return messages;
  return [...messages, message].slice(-HISTORY_MAX);
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
        messages: state.messages.map((message) =>
          message.id === event.id ? { ...message, reactions: event.reactions } : message,
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
 *  the snapshot's is already in it and is dropped; one that skips ahead marks
 *  the model stale so the caller refetches the state. An addressed event
 *  carries no sequence and always applies. */
export function applyPartyEvent(model: PartyModel, event: PartyEvent, now: number): PartyModel {
  if (!model.state) return model;
  if (event.type === 'party.sync.kicked') return { ...model, kicked: event.reason, kickedAt: now };
  if (event.seq <= model.seq) return model;
  const gap = model.seq > 0 && event.seq > model.seq + 1;
  const base: PartyModel = { ...model, seq: event.seq, stale: model.stale || gap };
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

export function expireFloating(model: PartyModel, now: number, ttlMs: number): PartyModel {
  const kept = model.floating.filter((entry) => now - entry.at < ttlMs);
  return kept.length === model.floating.length ? model : { ...model, floating: kept };
}

export const REACTIONS = [
  { emoji: '⚽', label: 'live.reaction.goal' },
  { emoji: '\u{1F525}', label: 'live.reaction.fire' },
  { emoji: '\u{1F602}', label: 'live.reaction.laugh' },
  { emoji: '\u{1F62E}', label: 'live.reaction.wow' },
  { emoji: '❤️', label: 'live.reaction.love' },
  { emoji: '\u{1F44F}', label: 'live.reaction.clap' },
] as const;

export function voteOf(
  vote: { votes: Record<string, boolean>; required: { userId: string }[] },
  userId: string,
): boolean | null {
  return vote.votes[userId] ?? null;
}

export function voteWaiting(vote: {
  votes: Record<string, boolean>;
  required: { userId: string }[];
}): number {
  return vote.required.filter((person) => !(person.userId in vote.votes)).length;
}

/** The vote this viewer still has to answer, or null: none open, already
 *  answered, put away, or their own request (a requester never votes on it). */
export function voteToAnswer(
  vote: VoteView | null,
  userId: string,
  dismissed: number | null,
): VoteView | null {
  if (!vote || userId === '' || vote.id === dismissed) return null;
  if (vote.by?.userId === userId || userId in vote.votes) return null;
  return vote;
}

/** The poll this viewer has not answered or put away. */
export function pollToAnswer(
  poll: PollView | null,
  userId: string,
  dismissed: number | null,
): PollView | null {
  if (!poll || userId === '' || poll.id === dismissed || userId in poll.votes) return null;
  return poll;
}
