import { ModuleFailed, ModuleLoading, useT } from '@kromatv/module-sdk';
import { Box, PageHeader } from '@kromatv/ui/kit';
import { AdminHealth } from './admin-health';
import { AdminProviders } from './admin-providers';
import { AdminScan } from './admin-scan';
import { useAdmin } from './use-admin';

export default function LiveAdminPage() {
  const t = useT();
  const { api, overview, history, metrics, failed, load } = useAdmin();
  if (!overview || !history || !metrics)
    return failed ? <ModuleFailed /> : <ModuleLoading panels={3} />;
  return (
    <>
      <PageHeader.Root>
        <PageHeader.Title>{t('live.admin.title')}</PageHeader.Title>
        <PageHeader.Subtitle>{t('live.admin.subtitle')}</PageHeader.Subtitle>
      </PageHeader.Root>
      <Box mt={24}>
        <AdminProviders api={api} overview={overview} onChanged={load} />
      </Box>
      <Box mt={20}>
        <AdminHealth api={api} history={history} metrics={metrics} onChanged={load} />
      </Box>
      <Box mt={20}>
        <AdminScan api={api} />
      </Box>
    </>
  );
}
