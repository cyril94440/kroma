import { defineModule } from '@kromatv/module-sdk';
import { lazy } from 'react';

const liveModule = defineModule({
  pages: [
    {
      path: 'live',
      component: lazy(() => import('./LivePage')),
      nav: { label: 'nav.live', icon: 'tv', section: 'library' },
    },
    {
      path: 'live-admin',
      component: lazy(() => import('./LiveAdminPage')),
      nav: {
        label: 'nav.liveAdmin',
        icon: 'tv',
        section: 'acquisition',
        requires: 'settings.manage',
      },
    },
  ],
});

export default liveModule;
