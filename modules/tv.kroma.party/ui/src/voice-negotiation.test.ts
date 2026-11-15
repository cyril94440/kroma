import { describe, expect, it } from 'vitest';
import { Negotiation } from './voice-negotiation';
import { SimNet, SimPeer } from './voice-peer-sim.fixture';

const handlers = {
  onCandidate: () => undefined,
  onNegotiationNeeded: () => undefined,
  onConnectionState: () => undefined,
  onStream: () => undefined,
};

function peer(): SimPeer {
  return new SimPeer(new SimNet(), handlers, 'x');
}

const offer = { type: 'offer', sdp: 'offer|rtc9|0' } as const;

describe('Negotiation', () => {
  it('answers an offer received while stable and remembers the answer', async () => {
    const negotiation = new Negotiation();
    const local = peer();

    const applied = await negotiation.apply(local, offer, false);

    expect(applied).toBe('answered');
    expect(negotiation.answer?.type).toBe('answer');
  });

  it('answers the same offer again without touching the connection', async () => {
    const negotiation = new Negotiation();
    const local = peer();
    await negotiation.apply(local, offer, false);
    const first = negotiation.answer;

    const applied = await negotiation.apply(local, offer, false);

    expect(applied).toBe('resent');
    expect(negotiation.answer).toBe(first);
  });

  it('ignores an offer that collides with its own for the impolite side', async () => {
    const negotiation = new Negotiation();
    const local = peer();
    await negotiation.makeOffer(local);

    const applied = await negotiation.apply(local, offer, false);

    expect(applied).toBe('ignored');
    expect(local.signalingState).toBe('have-local-offer');
  });

  it('rolls back its own offer to accept a colliding one on the polite side', async () => {
    const negotiation = new Negotiation();
    const local = peer();
    await negotiation.makeOffer(local);

    const applied = await negotiation.apply(local, offer, true);

    expect(applied).toBe('answered');
    expect(local.rollbacks).toBe(1);
  });

  it('accepts the answer to its own offer', async () => {
    const negotiation = new Negotiation();
    const local = peer();
    await negotiation.makeOffer(local);

    const applied = await negotiation.apply(local, { type: 'answer', sdp: 'answer|rtc9|0' }, false);

    expect(applied).toBe('accepted');
    expect(local.signalingState).toBe('stable');
  });

  it('reports a failure when the connection refuses the description', async () => {
    const negotiation = new Negotiation();
    const local = peer();

    const applied = await negotiation.apply(local, { type: 'answer', sdp: 'answer|rtc9|0' }, false);

    expect(applied).toBe('failed');
  });

  it('reports no offer when the connection is closed', async () => {
    const negotiation = new Negotiation();
    const local = peer();
    local.close();

    await expect(negotiation.makeOffer(local)).resolves.toBeNull();
  });

  it('drops candidates while it is ignoring the remote offer and forgets everything on reset', async () => {
    const negotiation = new Negotiation();
    const local = peer();
    await negotiation.makeOffer(local);
    await negotiation.apply(local, offer, false);

    await negotiation.addCandidate(local, { candidate: 'cand|rtc9|0|0' });
    negotiation.reset();

    expect(local.candidatesIn).toBe(0);
    expect(negotiation.answer).toBeNull();
  });
});
