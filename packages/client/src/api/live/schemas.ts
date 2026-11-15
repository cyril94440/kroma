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
  sourceResolution: z.string().default(''),
});
export type Status = z.infer<typeof Status>;

export const ChannelView = z.object({
  id: z.string(),
  name: z.string(),
  group: z.string().default(''),
  provider: Provider.nullable().default(null),
  score: z.number().default(0),
  now: Slot.nullable().default(null),
  event: z
    .object({ search: z.string().default(''), competition: z.string().default('') })
    .nullable()
    .catch(null)
    .default(null),
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
export type BeatAnswer = z.infer<typeof BeatAnswer>;

export const SwitchOutcome = z.object({
  status: z.string(),
  channel: ChannelRef.nullable().default(null),
  error: z.string().nullable().default(null),
  duration: z.number().nullable().default(null),
  restored: z.string().nullable().default(null),
});
export type SwitchOutcome = z.infer<typeof SwitchOutcome>;

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
