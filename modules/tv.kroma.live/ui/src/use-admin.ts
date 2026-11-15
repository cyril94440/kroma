import { useModuleFileUrl, useViewerApi } from '@kromatv/module-sdk';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { createLiveApi } from './live-api';
import type { History, Metrics, Overview } from './schemas';

const POLL_MS = 10_000;

export function useAdmin() {
  const viewer = useViewerApi();
  const fileUrl = useModuleFileUrl();
  const api = useMemo(() => createLiveApi(viewer, fileUrl), [viewer, fileUrl]);
  const [overview, setOverview] = useState<Overview | null>(null);
  const [history, setHistory] = useState<History | null>(null);
  const [metrics, setMetrics] = useState<Metrics | null>(null);
  const [failed, setFailed] = useState(false);

  const load = useCallback(() => {
    Promise.all([api.overview(), api.history(60), api.metrics()]).then(
      ([nextOverview, nextHistory, nextMetrics]) => {
        setOverview(nextOverview);
        setHistory(nextHistory);
        setMetrics(nextMetrics);
        setFailed(false);
      },
      () => setFailed(true),
    );
  }, [api]);

  useEffect(() => {
    load();
    const timer = setInterval(load, POLL_MS);
    return () => clearInterval(timer);
  }, [load]);

  return { api, overview, history, metrics, failed, load };
}
