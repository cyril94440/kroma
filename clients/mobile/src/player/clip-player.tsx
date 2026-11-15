import { usePreventRemove } from 'expo-router';
import { useEffect, useEffectEvent, useState } from 'react';
import { useT } from '#mobile/lib/i18n';
import { ClipChrome } from '#mobile/player/clip/clip-chrome';
import { ClipDetails } from '#mobile/player/clip/clip-details';
import { useClipEngine } from '#mobile/player/clip/use-clip-engine';
import { PlayerLayout } from '#mobile/player/player-layout';
import { PlayerStatusScreen } from '#mobile/player/player-status-screen';
import { PortraitHeader } from '#mobile/player/portrait-header';
import { PortraitPlaybackStrip } from '#mobile/player/portrait-playback-strip';
import { usePlayerPresentation } from '#mobile/player/use-player-presentation';
import { VideoSurface } from '#mobile/player/video-surface';

/** A short file the caller already holds a URL for, such as a goal clip, in the
 *  phone player's chrome. Nothing is saved and the end of the clip closes it. */
export function ClipPlayer({
  uri,
  title,
  subtitle,
  onClose,
}: Readonly<{ uri: string; title: string; subtitle: string; onClose(): void }>) {
  const t = useT();
  const clip = useClipEngine(uri, title);
  const presentation = usePlayerPresentation();
  const [fill, setFill] = useState(false);

  const [guarded, setGuarded] = useState(true);
  usePreventRemove(guarded, ({ repeat }) => {
    clip.shutdown();
    setGuarded(false);
    repeat();
  });

  const exit = () => {
    clip.shutdown();
    onClose();
  };
  const leaveAtEnd = useEffectEvent(exit);
  const ended = clip.endedNonce > 0;
  useEffect(() => {
    if (ended) leaveAtEnd();
  }, [ended]);

  if (clip.failed) {
    return <PlayerStatusScreen message={t('player.cantPlay')} onBack={exit} />;
  }

  return (
    <PlayerLayout
      fullscreen={presentation.fullscreen}
      header={<PortraitHeader onBack={exit} />}
      transport={<PortraitPlaybackStrip engine={clip} onFullscreen={presentation.toggle} />}
      details={<ClipDetails title={title} label={subtitle} />}
      video={
        <VideoSurface player={clip.player} fill={fill}>
          {({ startPip }) => (
            <ClipChrome
              clip={clip}
              title={title}
              subtitle={subtitle}
              fill={fill}
              onZoom={setFill}
              notice={null}
              compact={!presentation.fullscreen}
              onBack={presentation.fullscreen ? presentation.collapse : exit}
              onPip={startPip}
              onFullscreen={presentation.toggle}
            />
          )}
        </VideoSurface>
      }
    />
  );
}
