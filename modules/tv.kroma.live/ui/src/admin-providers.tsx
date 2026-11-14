import { useT } from '@kromatv/module-sdk';
import { Badge, Button, Callout, ListRow, Row, Section, Text } from '@kromatv/ui/kit';
import type { LiveApi } from './live-api';
import type { Overview } from './schemas';

interface AdminProvidersProps {
  api: LiveApi;
  overview: Overview;
  onChanged: () => void;
}

export function AdminProviders({ api, overview, onChanged }: Readonly<AdminProvidersProps>) {
  const t = useT();
  return (
    <Section.Root>
      <Section.Header>
        <Section.Title>{t('live.admin.providers')}</Section.Title>
      </Section.Header>
      {overview.providers.map((provider) => (
        <ListRow.Root key={provider.provider}>
          <ListRow.Label>{t(`live.provider.${provider.provider}`)}</ListRow.Label>
          <ListRow.Trailing>
            <Row gap={8}>
              {provider.singleConnection ? <Badge>{t('live.admin.singleConnection')}</Badge> : null}
              <Text variant="meta" color="textDim">
                {provider.configured
                  ? t('live.admin.channels', { count: provider.channels })
                  : t('live.admin.notConfigured')}
              </Text>
              {provider.error ? <Text variant="meta">{t('live.admin.failing')}</Text> : null}
            </Row>
          </ListRow.Trailing>
        </ListRow.Root>
      ))}
      <Row gap={8} mt={8}>
        <Text variant="meta" color="textDim">
          {t('live.admin.guide', { count: overview.guideProgrammes })}
        </Text>
        <Button
          label={t('live.admin.refresh')}
          variant="glass"
          size="sm"
          onPress={() => void api.refresh().then(onChanged)}
        />
      </Row>
      {overview.ffmpegHttps ? null : (
        <Callout.Root>
          <Text>{t('live.admin.noHttps')}</Text>
        </Callout.Root>
      )}
    </Section.Root>
  );
}
