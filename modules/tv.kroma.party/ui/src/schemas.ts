import { z } from 'zod';

export const Person = z.object({ userId: z.string(), name: z.string() });
export type Person = z.infer<typeof Person>;

export const Reactions = z.record(z.string(), z.array(Person));
export type Reactions = z.infer<typeof Reactions>;

export const ChatMessage = z.object({
  id: z.string(),
  userId: z.string().nullable().catch(null),
  name: z.string().catch(''),
  kind: z.enum(['user', 'system', 'agent']).catch('user'),
  text: z.string().catch(''),
  key: z.string().nullable().catch(null),
  params: z.record(z.string(), z.unknown()).catch({}),
  action: z.string().nullable().catch(null),
  card: z.unknown().nullable().catch(null),
  replyTo: z.string().nullable().catch(null),
  at: z.number(),
  reactions: Reactions.catch({}),
});
export type ChatMessage = z.infer<typeof ChatMessage>;

export const Tally = z.object({ yes: z.number(), no: z.number(), blank: z.number() });
export type Tally = z.infer<typeof Tally>;

export const VoteView = z.object({
  id: z.number(),
  kind: z.enum(['channel', 'provider']),
  label: z.string(),
  note: z.string().catch(''),
  by: Person.nullable(),
  endsAt: z.number(),
  endsInS: z.number(),
  required: z.array(Person),
  votes: z.record(z.string(), z.boolean()),
  tally: Tally,
  channelId: z.string().nullable(),
});
export type VoteView = z.infer<typeof VoteView>;

export const VoteClosed = z.object({
  id: z.number(),
  verdict: z.enum(['passed', 'refused', 'impossible', 'cancelled']),
  yes: z.number(),
  no: z.number(),
  blank: z.number(),
});
export type VoteClosed = z.infer<typeof VoteClosed>;

export const PollKind = z.enum(['yesno', 'choice', 'score', 'open']);

export const PollView = z.object({
  id: z.number(),
  question: z.string(),
  kind: PollKind,
  options: z.array(z.string()),
  votes: z.record(z.string(), z.string()),
  byAnswer: z.record(z.string(), z.array(z.string())),
  total: z.number(),
  endsAt: z.number(),
  endsInS: z.number(),
  closedBecause: z.string().optional(),
});
export type PollView = z.infer<typeof PollView>;

export const SyncState = z.object({
  target: z.number(),
  by: Person.nullable(),
  capped: z.boolean(),
  members: z.array(Person),
  active: z.number(),
});
export type SyncState = z.infer<typeof SyncState>;

export const Viewer = Person.extend({ since: z.number().catch(0) });
export type Viewer = z.infer<typeof Viewer>;

export const ViewerStats = z.object({
  name: z.string().catch(''),
  bufferS: z.number().catch(0),
  bandwidthKbps: z.number().catch(0),
  stalls: z.number().catch(0),
  rebufferMs: z.number().catch(0),
  dropped: z.number().catch(0),
  resolution: z.string().catch(''),
  playing: z.boolean().catch(false),
  delayS: z.number().nullable().catch(null),
  paused: z.boolean().catch(false),
  at: z.number().catch(0),
});

export const PartyState = z.object({
  seq: z.number(),
  now: z.number(),
  me: z.object({ userId: z.string(), name: z.string(), admin: z.boolean() }),
  messages: z.array(ChatMessage),
  vote: VoteView.nullable(),
  poll: PollView.nullable(),
  lastPoll: PollView.nullable().catch(null),
  activity: z.string().nullable(),
  sync: SyncState.nullable(),
  presence: z.array(Viewer),
  stats: z.record(z.string(), ViewerStats).catch({}),
  config: z.object({ voteSeconds: z.number(), agentName: z.string() }),
});
export type PartyState = z.infer<typeof PartyState>;

export const IDEA_GENRES = ['idea', 'bug', 'other'] as const;
export const IDEA_STATUSES = ['received', 'inProgress', 'done', 'refused'] as const;

export const Idea = z.object({
  id: z.string(),
  ts: z.number(),
  genre: z.enum(IDEA_GENRES).catch('other'),
  text: z.string(),
  status: z.enum(IDEA_STATUSES).catch('received'),
  reply: z.string().nullable().catch(null),
  author: Person,
  updatedAt: z.number().catch(0),
});
export type Idea = z.infer<typeof Idea>;

export const Ideas = z.object({ ideas: z.array(Idea) });
export const SentMessage = z.object({ message: ChatMessage });
export const SwitchOutcome = z.object({
  outcome: z.enum(['voteStarted', 'switchedDirectly', 'refused']),
  voteId: z.string().nullable(),
  reason: z.string().nullable(),
  forceDenied: z.boolean().catch(false),
});
export type SwitchOutcome = z.infer<typeof SwitchOutcome>;

export const Candidate = z.object({
  id: z.string(),
  name: z.string(),
  group: z.string().catch(''),
});
export const Candidates = z.object({ channels: z.array(Candidate) });
export type Candidate = z.infer<typeof Candidate>;

export const ServerTime = z.object({ now: z.number() });
export const Ok = z.object({ ok: z.boolean().catch(true) });
export const Created = z.object({ id: z.string() });
export const PollStarted = z.object({ poll: PollView });
export const SyncInfo = z.object({ active: z.boolean(), targetS: z.number().catch(0) });

const seq = { seq: z.number() };

export const PartyEvent = z.discriminatedUnion('type', [
  ChatMessage.extend({ type: z.literal('party.message'), ...seq }),
  z.object({ type: z.literal('party.reactions'), ...seq, id: z.string(), reactions: Reactions }),
  z.object({
    type: z.literal('party.reaction'),
    ...seq,
    userId: z.string(),
    name: z.string(),
    emoji: z.string(),
    at: z.number(),
  }),
  z.object({
    type: z.literal('party.vote'),
    ...seq,
    vote: VoteView.nullable(),
    closed: VoteClosed.nullable(),
  }),
  z.object({
    type: z.literal('party.poll'),
    ...seq,
    poll: PollView.nullable(),
    closed: PollView.nullable(),
  }),
  z.object({ type: z.literal('party.activity'), ...seq, label: z.string().nullable() }),
  z.object({ type: z.literal('party.sync'), ...seq, state: SyncState }),
  z.object({ type: z.literal('party.sync.golive'), ...seq }),
  z.object({ type: z.literal('party.sync.finesync'), ...seq }),
  z.object({
    type: z.literal('party.sync.kicked'),
    reason: z.string().catch(''),
    count: z.number().catch(0),
  }),
  z.object({ type: z.literal('party.ideas'), ...seq, ideas: z.array(Idea) }),
]);
export type PartyEvent = z.infer<typeof PartyEvent>;
