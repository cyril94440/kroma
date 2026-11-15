import { defineModule } from '@kromatv/module-sdk';
import { lazy } from 'react';

const sportModule = defineModule({
  pages: [
    {
      path: 'sport',
      component: lazy(() => import('./SportPage')),
      nav: { label: 'sport.nav', icon: 'trophy', section: 'library' },
    },
  ],
  slots: [
    {
      slot: 'tv.kroma.live/player.aside',
      component: lazy(() => import('./SportAside')),
      order: -1,
    },
  ],
});

export default sportModule;
