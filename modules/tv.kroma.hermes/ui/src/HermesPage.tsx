import { ModuleFailed, ModuleLoading, useT } from '@kromatv/module-sdk';
import { Box, PageHeader } from '@kromatv/ui/kit';
import { AuditPanel } from './audit-panel';
import { MemoryPanel } from './memory-panel';
import { ModelPanel } from './model-panel';
import { RulesPanel } from './rules-panel';
import { StatusPanel } from './status-panel';
import { useHermes } from './use-hermes';

export default function HermesPage() {
  const t = useT();
  const { api, status, failed, load } = useHermes();
  if (!status) return failed ? <ModuleFailed /> : <ModuleLoading panels={3} />;
  return (
    <>
      <PageHeader.Root>
        <PageHeader.Title>{t('hermes.title')}</PageHeader.Title>
        <PageHeader.Subtitle>{t('hermes.subtitle')}</PageHeader.Subtitle>
      </PageHeader.Root>
      <Box mt={24}>
        <StatusPanel api={api} status={status} onChanged={load} />
      </Box>
      <Box mt={20}>
        <ModelPanel api={api} />
      </Box>
      <Box mt={20}>
        <RulesPanel api={api} />
      </Box>
      <Box mt={20}>
        <MemoryPanel api={api} />
      </Box>
      <Box mt={20}>
        <AuditPanel api={api} />
      </Box>
    </>
  );
}
