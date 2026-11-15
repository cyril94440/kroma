import { defineModule } from '@kromatv/module-sdk';
import { lazy } from 'react';

const partyModule = defineModule({
  pages: [
    {
      path: 'party',
      component: lazy(() => import('./PartyPage')),
      nav: { label: 'nav.party', icon: 'chat', section: 'library' },
    },
  ],
  slots: [
    { slot: 'tv.kroma.live/player.aside', component: lazy(() => import('./PartyAside')) },
    { slot: 'tv.kroma.live/player.overlay', component: lazy(() => import('./PartyOverlay')) },
  ],
});

export default partyModule;
