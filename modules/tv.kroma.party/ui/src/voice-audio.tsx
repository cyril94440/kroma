import { useEffect, useRef } from 'react';

interface RemoteAudioProps {
  stream: MediaStream;
  muted: boolean;
  retry: number;
  onBlocked: (blocked: boolean) => void;
}

/** Plays one remote voice. A browser may refuse to start audio without a
 *  gesture, which is reported so the panel can offer a button; `retry` changes
 *  when the person presses it. */
export function RemoteAudio({ stream, muted, retry, onBlocked }: Readonly<RemoteAudioProps>) {
  const element = useRef<HTMLAudioElement>(null);
  useEffect(() => {
    const audio = element.current;
    if (!audio) return;
    audio.srcObject = stream;
    audio.play().then(
      () => onBlocked(false),
      () => onBlocked(true),
    );
  }, [stream, retry, onBlocked]);
  // oxlint-disable-next-line jsx-a11y/media-has-caption -- a live voice from someone in the room, there is no transcript to caption.
  return <audio ref={element} muted={muted} autoPlay playsInline data-testid="voice-audio" />;
}
