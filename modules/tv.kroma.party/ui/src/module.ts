import { defineModule } from '@kromatv/module-sdk';
import { lazy } from 'react';

const partyModule = defineModule({
  pages: [
    {
      path: 'party',
      component: lazy(() => import('./PartyPage')),
      nav: { label: 'nav.party', icon: 'tv', section: 'library' },
    },
  ],
});

export default partyModule;
