import { useT } from '@kromatv/module-sdk';
import { Badge, Button, Icon, Row, Section, Text } from '@kromatv/ui/kit';
import { useCallback, useState } from 'react';
import { RemoteAudio } from './voice-audio';
import { useVoice } from './use-voice';
import type { VoiceView } from './voice-view';

const FAILURE_KEYS = {
  insecure: 'party.voice.error.insecure',
  denied: 'party.voice.error.denied',
  missing: 'party.voice.error.missing',
  timeout: 'party.voice.error.timeout',
  unavailable: 'party.voice.error.unavailable',
  full: 'party.voice.error.full',
  rateLimited: 'party.voice.error.rateLimited',
  failed: 'party.voice.error.failed',
} as const;

function Roster({ view }: Readonly<{ view: VoiceView }>) {
  const t = useT();
  if (!view.participants.length) {
    return (
      <Text variant="meta" color="textDim" mt={6}>
        {t('party.voice.empty')}
      </Text>
    );
  }
  return (
    <>
      {view.participants.map((member) => {
        const speaking =
          member.peerId === view.peerId ? view.talking : view.speaking.has(member.peerId);
        const link = view.links.get(member.peerId);
        return (
          <Row key={member.peerId} gap={8} mt={6} data-testid={`voice-member-${member.peerId}`}>
            <Icon
              name={member.muted ? 'microphone-off' : 'microphone'}
              size={18}
              color={speaking ? 'accent' : 'textDim'}
            />
            <Text variant="label">{member.name}</Text>
            {speaking ? (
              <Text variant="meta" color="accent">
                {t('party.voice.speaking')}
              </Text>
            ) : null}
            {member.peerId === view.peerId ? <Badge>{t('party.voice.you')}</Badge> : null}
            {link === 'connecting' ? (
              <Text variant="meta" color="textDim">
                {t('party.voice.connecting')}
              </Text>
            ) : null}
            {link === 'failed' ? (
              <Text variant="meta" color="textDim">
                {t('party.voice.linkFailed')}
              </Text>
            ) : null}
          </Row>
        );
      })}
    </>
  );
}

interface ControlsProps {
  view: VoiceView;
  deafened: boolean;
  onJoin: () => void;
  onLeave: () => void;
  onMute: () => void;
  onDeafen: () => void;
}

function Controls({ view, deafened, onJoin, onLeave, onMute, onDeafen }: Readonly<ControlsProps>) {
  const t = useT();
  const live = view.phase === 'live';
  return (
    <Row wrap gap={8}>
      <Button
        label={t(live ? 'party.voice.leave' : 'party.voice.join')}
        icon={live ? 'phone-off' : 'phone-call'}
        variant={live ? 'glass' : 'primary'}
        size="sm"
        loading={view.phase === 'joining'}
        onPress={live ? onLeave : onJoin}
      />
      {live ? (
        <>
          <Button
            label={t(view.muted ? 'party.voice.unmute' : 'party.voice.mute')}
            icon={view.muted ? 'microphone-off' : 'microphone'}
            variant="glass"
            size="sm"
            pressed={view.muted}
            onPress={onMute}
          />
          <Button
            label={t(deafened ? 'party.voice.soundOn' : 'party.voice.soundOff')}
            icon={deafened ? 'volume-off' : 'volume'}
            variant="glass"
            size="sm"
            pressed={deafened}
            onPress={onDeafen}
          />
        </>
      ) : null}
    </Row>
  );
}

export function VoicePanel({ watching }: Readonly<{ watching: boolean }>) {
  const t = useT();
  const { view, supported, join, leave, toggleMute } = useVoice(watching);
  const [deafened, setDeafened] = useState(false);
  const [blocked, setBlocked] = useState(false);
  const [retry, setRetry] = useState(0);
  const unlock = useCallback(() => setRetry((n) => n + 1), []);
  const toggleDeafened = useCallback(() => setDeafened((on) => !on), []);

  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('party.voice.title')}</Section.Title>
      </Section.Header>
      {supported ? (
        <Controls
          view={view}
          deafened={deafened}
          onJoin={join}
          onLeave={leave}
          onMute={toggleMute}
          onDeafen={toggleDeafened}
        />
      ) : (
        <Text variant="meta" color="textDim" data-testid="voice-insecure">
          {t('party.voice.insecure')}
        </Text>
      )}
      {view.failure ? (
        <Text variant="meta" mt={6} data-testid="voice-failure">
          {t(FAILURE_KEYS[view.failure])}
        </Text>
      ) : null}
      {blocked ? (
        <Button
          label={t('party.voice.enableSound')}
          icon="volume"
          variant="glass"
          size="sm"
          onPress={unlock}
        />
      ) : null}
      <Roster view={view} />
      {[...view.streams].map(([peerId, stream]) => (
        <RemoteAudio
          key={peerId}
          stream={stream}
          muted={deafened}
          retry={retry}
          onBlocked={setBlocked}
        />
      ))}
    </Section.Root>
  );
}
