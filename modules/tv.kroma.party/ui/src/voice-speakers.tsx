import { useT } from '@kromatv/module-sdk';
import { Badge, Row } from '@kromatv/ui/kit';
import { useVoice } from './use-voice';

export function VoiceSpeakers() {
  const t = useT();
  const { view } = useVoice(true);
  const talking = view.participants.filter((member) => view.speaking.has(member.peerId));
  if (!talking.length) return null;
  return (
    <Row gap={6} wrap data-testid="voice-speakers" aria-label={t('party.voice.title')}>
      {talking.map((member) => (
        <Badge key={member.peerId}>{member.name}</Badge>
      ))}
    </Row>
  );
}
