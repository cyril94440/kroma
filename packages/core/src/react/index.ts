// React bindings for the shared domain logic - `@kromatv/core/react`. The
// domain layer (`@kromatv/core`) is deliberately React-free, so headless React
// logic that every client drives the same way lives here instead, parameterized
// over a HOST adapter for the pieces that differ per app (session, client
// instance). No components or styles - that is @kromatv/ui's floor.

export type { AskedTitle, TitleAsk } from './ask-for-title';
export { useAskForTitle } from './ask-for-title';
export type {
  CheckPrompt,
  CheckPromptOptions,
  HandoffAttempt,
  HandoffOutcome,
  HandoffPicker,
  HandoffPickerOptions,
} from './handoff-picker';
export {
  HANDOFF_OUTCOME_MS,
  handoffRowHint,
  useCheckPrompt,
  useHandoffPicker,
} from './handoff-picker';
export type { LangPatch, LangPrefs, LangPrefsHost, LangPrefUser } from './lang-prefs';
export { normalizeLangPref, prefValue, useLangPrefs } from './lang-prefs';
export type { NearbyTvs } from './nearby-tvs';
export { useNearbyTvs } from './nearby-tvs';
export type { HeldSeekTransport } from './seek-hold';
export { useHeldSeek } from './seek-hold';
export type { TrailerState } from './trailer-preparation';
export { useTrailerPreparation } from './trailer-preparation';
export type { TitleWatchNow, WatchedTitle, WatchNowOptions, WatchNowState } from './watch-now';
export { useWatchNow } from './watch-now';
export type { EventHandlers, OpenEvents } from './live-events';
export { socketEvents } from './live-events';
export type { LiveClient, LiveRoom, LiveRoomOptions } from './use-live-room';
export {
  DEFAULT_HEARTBEAT_MS,
  FLOATING_TTL_MS,
  SAFETY_REFRESH_MS,
  useLiveRoom,
} from './use-live-room';
export type { LiveModulesState } from './use-live-modules';
export { useLiveModules } from './use-live-modules';
