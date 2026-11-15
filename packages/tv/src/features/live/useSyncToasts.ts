import type { LiveSync } from '@kromatv/core/react';
import { useT } from '@kromatv/ui';
import { toast } from '@kromatv/ui/kit';
import { useEffect } from 'react';

/** Says what the watch party sync just did, each notice once. */
export function useSyncToasts(notice: LiveSync['notice']): void {
  const t = useT();
  useEffect(() => {
    if (notice) toast({ message: t(notice.key, notice.vars) });
  }, [notice, t]);
}
