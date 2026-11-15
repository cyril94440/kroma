import type { Idea } from '@kromatv/client/party';
import { useT } from '@kromatv/ui';
import { Badge, Box, Drawer, Text } from '@kromatv/ui/kit';

const IDEAS_SHOWN = 10;

/** The family's ideas with the answer each one got. Writing one takes a
 *  keyboard, so a television only reads the box and points to the phone. */
export function IdeasDrawer({
  open,
  ideas,
  onClose,
}: Readonly<{ open: boolean; ideas: readonly Idea[]; onClose: () => void }>) {
  const t = useT();
  return (
    <Drawer.Root open={open} onClose={onClose} title={t('live.ideas')} width="lg" floating>
      <Drawer.Panel>
        <Box gap={16}>
          <Text variant="labelTv" color="textDim">
            {t('live.ideas.hint')}
          </Text>
          {ideas.length === 0 ? (
            <Text variant="bodyTv" color="textDim">
              {t('live.ideas.empty')}
            </Text>
          ) : null}
          {ideas.slice(0, IDEAS_SHOWN).map((idea) => (
            <Box key={idea.id} gap={6}>
              <Box row gap={12} align="center">
                <Text variant="labelTv" color="accentText">
                  {idea.author.name}
                </Text>
                <Badge>{t(`live.ideas.status.${idea.status}`)}</Badge>
              </Box>
              <Text variant="bodyTv">{idea.text}</Text>
              {idea.reply ? (
                <Text variant="labelTv" color="textMuted">
                  {idea.reply}
                </Text>
              ) : null}
            </Box>
          ))}
        </Box>
      </Drawer.Panel>
    </Drawer.Root>
  );
}
