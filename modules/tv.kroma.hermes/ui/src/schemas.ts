import { z } from 'zod';

export const AssistantStatus = z.object({
  enabled: z.boolean().default(false),
  paused: z.boolean().default(false),
  provider: z.string().default(''),
  model: z.string().default(''),
  configured: z.boolean().default(false),
  busy: z.boolean().default(false),
  speakingRecently: z.boolean().default(false),
  voteOpen: z.boolean().default(false),
  pollOpen: z.boolean().default(false),
  callsLastHour: z.number().default(0),
  hourlyLimit: z.number().default(0),
});
export type AssistantStatus = z.infer<typeof AssistantStatus>;

export const AuditRow = z.object({
  id: z.number(),
  at: z.number(),
  genre: z.string(),
  event: z.string(),
  data: z.unknown().optional(),
});
export type AuditRow = z.infer<typeof AuditRow>;

export const AuditPage = z.object({
  rows: z.array(AuditRow).default([]),
  genres: z.array(z.string()).default([]),
});
export type AuditPage = z.infer<typeof AuditPage>;

export const Rules = z.object({
  text: z.string(),
  default: z.string().default(''),
  customised: z.boolean().default(false),
});
export type Rules = z.infer<typeof Rules>;

export const Lesson = z.object({ id: z.number(), at: z.number(), text: z.string() });
export const Lessons = z.object({ lessons: z.array(Lesson).default([]) });

export const Suggestion = z.object({
  id: z.number(),
  at: z.number(),
  kind: z.string(),
  title: z.string(),
  detail: z.string(),
});
export const Suggestions = z.object({ suggestions: z.array(Suggestion).default([]) });

export const PersonNote = z.object({ userId: z.string(), name: z.string(), note: z.string() });
export type PersonNote = z.infer<typeof PersonNote>;
export const Notes = z.object({ notes: z.array(PersonNote).default([]) });

export const ModelTest = z.object({
  ok: z.boolean(),
  error: z.string().optional(),
  provider: z.string().optional(),
  model: z.string().optional(),
  answer: z.string().optional(),
  ms: z.number().optional(),
  search: z.boolean().optional(),
});
export type ModelTest = z.infer<typeof ModelTest>;

export const ModelList = z.object({
  ok: z.boolean(),
  error: z.string().optional(),
  models: z.array(z.string()).default([]),
});
export type ModelList = z.infer<typeof ModelList>;

export const Done = z.object({ ok: z.boolean().optional() }).loose();
