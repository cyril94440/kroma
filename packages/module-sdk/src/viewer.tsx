import type { ServerEvent } from '@kromatv/client/events';
import { type ModuleApi, ModuleId, type ModuleInfo } from '@kromatv/client/modules';
import { createContext, type ReactNode, useContext, useMemo } from 'react';
import { ModuleIdContext } from './scope-id';

/** What the shell lends a module page or slot that is not under the admin
 *  console. Each call is bound to one module id, so a page reaches its own
 *  `/api/module/<id>` mount and its own published files and nothing else. */
export interface ViewerHostValue {
  moduleApi(id: ModuleId): ModuleApi;
  fileUrl(id: ModuleId, path: string): string;
  listModules(): Promise<ModuleInfo[]>;
  openEvents(handlers: ViewerEventHandlers): () => void;
}

export interface ViewerEventHandlers {
  onEvent: (event: ServerEvent | { type: string }) => void;
  onOpen?: () => void;
}

const ViewerHostContext = createContext<ViewerHostValue | null>(null);

/** Mounted once by a shell that renders module pages for signed-in accounts. */
export function ViewerHostProvider({
  value,
  children,
}: Readonly<{ value: ViewerHostValue; children: ReactNode }>) {
  return <ViewerHostContext.Provider value={value}>{children}</ViewerHostContext.Provider>;
}

export function useOptionalViewerHost(): ViewerHostValue | null {
  return useContext(ViewerHostContext);
}

function useOwnId(hook: string): ModuleId {
  const id = useContext(ModuleIdContext);
  if (id === null) throw new Error(`${hook} must be used inside a module page`);
  return id;
}

function useViewerHost(hook: string): ViewerHostValue {
  const host = useContext(ViewerHostContext);
  if (!host) throw new Error(`${hook} needs a shell that mounts <ViewerHostProvider>`);
  return host;
}

/** This module's own routes for any signed-in account, bound to
 *  `/api/module/<its id>`. Paths are relative to that mount. Credentials and
 *  their refresh are the shell client's: a module never holds a token. */
export function useViewerApi(): ModuleApi {
  const id = useOwnId('useViewerApi');
  const host = useViewerHost('useViewerApi');
  return useMemo(() => host.moduleApi(id), [host, id]);
}

/** The URL of a file THIS module published, carrying the media ticket a
 *  `<video>` or hls.js cannot send as a header. */
export function useModuleFileUrl(): (path: string) => string {
  const id = useOwnId('useModuleFileUrl');
  const host = useViewerHost('useModuleFileUrl');
  return useMemo(() => (path) => host.fileUrl(id, path), [host, id]);
}
