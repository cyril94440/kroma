import { z } from 'zod';

export const Phase = z.enum(['paused', 'running', 'switching', 'offAir', 'error']);
export type Phase = z.infer<typeof Phase>;

export const Provider = z.enum(['primary', 'backup']);

export const ChannelRef = z.object({ id: z.string(), name: z.string() });
export type ChannelRef = z.infer<typeof ChannelRef>;

export const Slot = z.object({
  title: z.string(),
  start: z.number(),
  end: z.number(),
});
export type Slot = z.infer<typeof Slot>;

export const Viewer = z.object({
  userId: z.string(),
  name: z.string(),
  since: z.number().default(0),
});
export type Viewer = z.infer<typeof Viewer>;

export const Health = z.object({
  sourceKbps: z.number().default(0),
  stutter: z.boolean().default(false),
  black: z.boolean().default(false),
  frozen: z.boolean().default(false),
  trouble: z.boolean().default(false),
  state: z.string().default('starting'),
  lossSeconds: z.number().default(0),
});
export type Health = z.infer<typeof Health>;

export const OffAir = z.object({
  name: z.string(),
  duration: z.number().nullable().default(null),
  at: z.number().default(0),
});

export const Status = z.object({
  state: Phase,
  channel: ChannelRef.nullable(),
  provider: Provider,
  onBackup: z.boolean().default(false),
  now: Slot.nullable().default(null),
  next: Slot.nullable().default(null),
  viewers: z.array(Viewer).default([]),
  health: Health.default(() => Health.parse({})),
  playlist: z.string().nullable().default(null),
  startedAt: z.number().nullable().default(null),
  restarts: z.number().default(0),
  standbyInSeconds: z.number().nullable().default(null),
  lastError: z.string().nullable().default(null),
  offAir: OffAir.nullable().default(null),
  backupConfigured: z.boolean().default(false),
  primaryDown: z.boolean().default(false),
});
export type Status = z.infer<typeof Status>;

export const ChannelView = z.object({
  id: z.string(),
  name: z.string(),
  group: z.string().default(''),
  provider: Provider.nullable().default(null),
  score: z.number().default(0),
  now: Slot.nullable().default(null),
});
export type ChannelView = z.infer<typeof ChannelView>;

export const ChannelPage = z.object({
  total: z.number(),
  page: z.number(),
  perPage: z.number(),
  channels: z.array(ChannelView),
  groups: z.array(z.string()).default([]),
});
export type ChannelPage = z.infer<typeof ChannelPage>;

export const Favourites = z.array(ChannelView);

export const BeatAnswer = z.object({ viewers: z.number(), heartbeatSeconds: z.number() });

export const SwitchOutcome = z.object({
  status: z.string(),
  channel: ChannelRef.nullable().default(null),
  error: z.string().nullable().default(null),
  duration: z.number().nullable().default(null),
  restored: z.string().nullable().default(null),
});
export type SwitchOutcome = z.infer<typeof SwitchOutcome>;

export const SourceView = z.object({
  provider: Provider,
  configured: z.boolean(),
  channels: z.number(),
  fetchedAt: z.number(),
  error: z.string().nullable().default(null),
  singleConnection: z.boolean().default(false),
});

export const Overview = z.object({
  providers: z.array(SourceView),
  guideProgrammes: z.number().default(0),
  guideFetchedAt: z.number().default(0),
  guideSource: z.string().default(''),
  guideError: z.string().nullable().default(null),
  ffmpegHttps: z.boolean().default(false),
  favourites: z.array(z.string()).default([]),
  standbySeconds: z.number().default(90),
});
export type Overview = z.infer<typeof Overview>;

export const HistoryPoint = z.object({
  at: z.number(),
  kbps: z.number().default(0),
  channel: z.string().nullable().default(null),
  provider: z.string().nullable().default(null),
  health: z.string().nullable().default(null),
  loss: z.number().default(0),
  frozen: z.boolean().default(false),
  black: z.boolean().default(false),
  restarts: z.number().default(0),
  paused: z.boolean().default(false),
});

export const ProviderEvent = z.object({
  id: z.number(),
  at: z.number(),
  kind: z.string(),
  from: z.string().nullable().default(null),
  to: z.string().nullable().default(null),
  channel: z.string().nullable().default(null),
  detail: z.string().default(''),
});

export const History = z.object({
  stepSeconds: z.number(),
  points: z.array(HistoryPoint),
  providerEvents: z.array(ProviderEvent).default([]),
});
export type History = z.infer<typeof History>;

export const Metrics = z.object({
  zaps: z.number(),
  lastMs: z.number().nullable(),
  p50Ms: z.number().nullable(),
  p95Ms: z.number().nullable(),
  restartsTotal: z.number().default(0),
});
export type Metrics = z.infer<typeof Metrics>;

export const ScanItem = z.object({
  number: z.number().nullable().default(null),
  id: z.string(),
  name: z.string(),
  ok: z.boolean().default(false),
  black: z.boolean().default(false),
  frame: z.string().nullable().default(null),
  error: z.string().nullable().default(null),
  now: z.string().nullable().default(null),
});
export type ScanItem = z.infer<typeof ScanItem>;

export const ScanState = z.object({
  id: z.string().default(''),
  query: z.string().default(''),
  running: z.boolean().default(false),
  total: z.number().default(0),
  items: z.array(ScanItem).default([]),
});
export type ScanState = z.infer<typeof ScanState>;

export const Started = z.object({ id: z.string() });
export const Refreshed = z.record(z.string(), z.unknown());

export const LiveEvent = z.discriminatedUnion('type', [
  Status.extend({ type: z.literal('live.status') }),
  z.object({ type: z.literal('live.presence'), viewers: z.array(Viewer) }),
  Health.extend({ type: z.literal('live.health') }),
  z.object({
    type: z.literal('live.proposal'),
    resolved: z.boolean().default(false),
    accepted: z.boolean().optional(),
  }),
  z.object({ type: z.literal('live.failover'), kind: z.string() }),
  z.object({ type: z.literal('live.scan'), id: z.string() }),
]);
export type LiveEvent = z.infer<typeof LiveEvent>;
