import {
  capabilities,
  type DecoderFrameLimits,
  type HdrSupport,
  type PlaybackCapabilities,
  withHdrSupport,
} from '@kromatv/core';
import { createContext, type ReactNode, useContext, useMemo } from 'react';

const Context = createContext<PlaybackCapabilities>(capabilities());
/** Decoder ceilings belong to this television, including its remounted player. */
export function PlaybackCapabilitiesProvider({
  limits,
  hdrSupport,
  children,
}: {
  readonly limits?: DecoderFrameLimits;
  readonly hdrSupport?: HdrSupport;
  readonly children: ReactNode;
}) {
  const value = useMemo(() => {
    const caps = withHdrSupport(capabilities(), hdrSupport);
    return limits ? { ...caps, frameLimits: limits } : caps;
  }, [limits, hdrSupport]);
  return <Context.Provider value={value}>{children}</Context.Provider>;
}
export function usePlaybackCapabilities() {
  return useContext(Context);
}
