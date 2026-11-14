import { z } from 'zod';
import type { RequestContext } from '../../core/client';
import type { ModuleId } from './ids';
import { queryString } from '../../core/http/query';
import { moduleApi } from './module-api';
import { ModuleInfo } from './schemas';

const Relayed = z.array(z.unknown()).transform((entries) =>
  entries.flatMap((entry) => {
    const parsed = ModuleInfo.safeParse(entry);
    return parsed.success ? [parsed.data] : [];
  }),
);

/** The module registry, and the door to one module's own admin API. */
export default function modulesApi(ctx: RequestContext) {
  return {
    /** Modules on this server with their enabled flag + capabilities; drives the
     * admin console's data-driven ADD flows. */
    list: () => ctx.get('/modules', Relayed),

    /** The admin API of one module, addressed by its id. A module's routes are
     * its own: they live under `/api/admin/m/<id>`, so they are not part of this
     * facade and cannot collide with a core route. */
    api: (id: ModuleId) => moduleApi(ctx, id),

    /** The routes module `id` serves to any signed-in account, under
     * `/api/module/<id>`; the module decides who may do what. */
    viewerApi: (id: ModuleId) => moduleApi(ctx, id, 'viewer'),

    /** URL of a file module `id` published (`/api/module-files/<id>/<path>`),
     * carrying the account's media ticket because a `<video>`, an hls.js
     * request or a native player cannot send a header. A playlist fetched
     * through it comes back with the ticket on every URI. */
    fileUrl: (id: ModuleId, path: string) => {
      const segments = path.split('/').map(encodeURIComponent).join('/');
      const query = queryString({ t: ctx.mediaTicket() });
      return `${ctx.baseUrl}/api/module-files/${encodeURIComponent(id)}/${segments}${query}`;
    },
  };
}

declare module '../../core/client' {
  interface Domains {
    modules: ReturnType<typeof modulesApi>;
  }
}
