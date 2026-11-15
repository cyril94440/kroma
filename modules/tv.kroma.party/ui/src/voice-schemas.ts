import { z } from 'zod';

export const VoiceMember = z.object({
  peerId: z.string(),
  userId: z.string(),
  name: z.string().catch(''),
  muted: z.boolean().catch(false),
});
export type VoiceMember = z.infer<typeof VoiceMember>;

export const IceServer = z.object({
  urls: z.array(z.string()),
  username: z.string().optional(),
  credential: z.string().optional(),
});
export type IceServer = z.infer<typeof IceServer>;

export const VoiceState = z.object({
  revision: z.number(),
  participants: z.array(VoiceMember),
  speaking: z.array(z.string()).catch([]),
});
export type VoiceState = z.infer<typeof VoiceState>;

export const JoinAnswer = VoiceState.extend({
  peerId: z.string(),
  iceServers: z.array(IceServer).catch([]),
  beatSeconds: z.number().catch(10),
  expireSeconds: z.number().catch(30),
});
export type JoinAnswer = z.infer<typeof JoinAnswer>;

export const IceAnswer = z.object({ iceServers: z.array(IceServer).catch([]) });
export const BeatAnswer = z.object({ revision: z.number() });
export const SignalAnswer = z.object({ n: z.number().catch(0) });

export const SessionDescription = z.object({
  type: z.enum(['offer', 'answer']),
  sdp: z.string(),
});
export type SessionDescription = z.infer<typeof SessionDescription>;

export const IceCandidate = z.object({
  candidate: z.string(),
  sdpMid: z.string().nullish(),
  sdpMLineIndex: z.number().nullish(),
  usernameFragment: z.string().nullish(),
});
export type IceCandidate = z.infer<typeof IceCandidate>;

const route = { from: z.string(), to: z.string(), session: z.string() };

export const SignalBody = z.discriminatedUnion('kind', [
  z.object({ ...route, kind: z.literal('description'), description: SessionDescription }),
  z.object({ ...route, kind: z.literal('candidate'), candidate: IceCandidate }),
  z.object({ ...route, kind: z.literal('hello') }),
]);
export type SignalBody = z.infer<typeof SignalBody>;

const signalExtras = {
  type: z.literal('party.voice.signal'),
  n: z.number(),
  fromUserId: z.string(),
};

export const SignalEvent = z.discriminatedUnion('kind', [
  z.object({
    ...route,
    ...signalExtras,
    kind: z.literal('description'),
    description: SessionDescription,
  }),
  z.object({ ...route, ...signalExtras, kind: z.literal('candidate'), candidate: IceCandidate }),
  z.object({ ...route, ...signalExtras, kind: z.literal('hello') }),
]);
export type SignalEvent = z.infer<typeof SignalEvent>;

export const RosterEvent = z.object({
  type: z.literal('party.voice.roster'),
  revision: z.number(),
  participants: z.array(VoiceMember),
});
export type RosterEvent = z.infer<typeof RosterEvent>;

export const SpeakingEvent = z.object({
  type: z.literal('party.voice.speaking'),
  speaking: z.array(z.string()),
});
export type SpeakingEvent = z.infer<typeof SpeakingEvent>;

export const VoiceEvent = z.union([RosterEvent, SpeakingEvent, SignalEvent]);
export type VoiceEvent = z.infer<typeof VoiceEvent>;
