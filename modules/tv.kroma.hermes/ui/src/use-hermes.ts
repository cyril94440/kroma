import type { KromaHost } from '@kromatv/module-sdk';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { createHermesApi } from './hermes-api';
import type { AssistantStatus } from './schemas';

const POLL_MS = 5000;

export function useHermes(host: KromaHost) {
  const api = useMemo(() => createHermesApi(host), [host]);
  const [status, setStatus] = useState<AssistantStatus | null>(null);
  const [failed, setFailed] = useState(false);

  const load = useCallback(() => {
    api.status().then(
      (next) => {
        setStatus(next);
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

  return { api, status, failed, load };
}
