import type { ModuleId } from '@kromatv/client/modules';
import { createContext } from 'react';

export const ModuleIdContext = createContext<ModuleId | null>(null);
