import { defineModule } from '@kromatv/module-sdk';
import { lazy } from 'react';

const hermesModule = defineModule({
  pages: [
    {
      path: 'hermes',
      component: lazy(() => import('./HermesPage')),
      nav: { label: 'nav.hermes', icon: 'tv', section: 'system', requires: 'settings.manage' },
    },
  ],
});

export default hermesModule;
