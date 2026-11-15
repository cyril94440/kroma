import { useServerEvents, useViewerApi } from '@kromatv/module-sdk';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { createVoiceApi } from './voice-api';
import { deviceId, newSessionId } from './voice-device';
import { acquireMic, voiceSupported } from './voice-mic';
import { createMeterAudio, watchSpeech } from './voice-meter';
import { createRtcPeer } from './voice-rtc';
import { VoiceEvent } from './voice-schemas';
import { VoiceSession } from './voice-session';
import { IDLE, type VoiceView } from './voice-view';

interface VoiceControls {
  view: VoiceView;
  supported: boolean;
  join: () => void;
  leave: () => void;
  toggleMute: () => void;
}

export function useVoice(watching: boolean): VoiceControls {
  const viewer = useViewerApi();
  const [view, setView] = useState<VoiceView>(IDLE);
  const audio = useMemo(() => createMeterAudio(), []);
  const supported = useMemo(() => voiceSupported(), []);
  const session = useMemo(
    () =>
      new VoiceSession({
        api: createVoiceApi(viewer),
        deviceId: deviceId(),
        watching,
        acquireMic,
        watchSpeech: (stream, onChange) => watchSpeech(stream, onChange, audio),
        createPeer: createRtcPeer,
        newSession: newSessionId,
        now: Date.now,
        onChange: setView,
      }),
    [viewer, watching, audio],
  );

  useEffect(() => {
    void session.observe();
    const unload = () => session.leaveOnUnload();
    window.addEventListener('pagehide', unload);
    return () => {
      window.removeEventListener('pagehide', unload);
      session.dispose();
    };
  }, [session]);

  useServerEvents<{ type: string }>(
    (raw) => {
      if (!raw.type.startsWith('party.voice.')) return;
      const parsed = VoiceEvent.safeParse(raw);
      if (parsed.success) session.handleEvent(parsed.data);
    },
    () => void session.refresh(),
  );

  const join = useCallback(() => {
    audio.prime();
    void session.join();
  }, [audio, session]);
  const leave = useCallback(() => void session.leave(), [session]);
  const toggleMute = useCallback(() => void session.setMuted(!session.snapshot.muted), [session]);
  return { view, supported, join, leave, toggleMute };
}
