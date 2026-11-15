import type { Idea } from '@kromatv/client/party';
import { useCallback, useEffect, useState } from 'react';
import type { LiveClient } from './use-live-room';

/** The family's ideas with their statuses, read again whenever the box changes
 *  (`version` is the party's ideas sequence). A failed read leaves what is shown. */
export function useIdeas(client: Pick<LiveClient, 'party'>, version: number, enabled: boolean) {
  const [ideas, setIdeas] = useState<Idea[]>([]);
  const reload = useCallback(() => {
    client.party
      .ideas()
      .then((answer) => setIdeas(answer.ideas))
      .catch(() => undefined);
  }, [client]);
  useEffect(() => {
    if (enabled) reload();
  }, [reload, version, enabled]);
  return { ideas, reload };
}
