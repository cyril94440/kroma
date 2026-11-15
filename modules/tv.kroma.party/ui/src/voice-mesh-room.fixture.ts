import { SimNet } from './voice-peer-sim.fixture';
import { SimServer } from './voice-sim.fixture';

export const MAMIE = { userId: 'mamie', name: 'Mamie' };
export const PAPI = { userId: 'papi', name: 'Papi' };
export const CYRIL = { userId: 'cyril', name: 'Cyril' };

export function room() {
  const server = new SimServer();
  const net = new SimNet();
  return { server, net };
}
