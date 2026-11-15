import { defineModule } from '@kromatv/module-sdk';
import { lazy } from 'react';

const sportModule = defineModule({
  pages: [
    {
      path: 'sport',
      component: lazy(() => import('./SportPage')),
      nav: { label: 'sport.nav', icon: 'chart', section: 'library' },
    },
  ],
  slots: [{ slot: 'tv.kroma.live/player.aside', component: lazy(() => import('./SportAside')) }],
});

export default sportModule;
