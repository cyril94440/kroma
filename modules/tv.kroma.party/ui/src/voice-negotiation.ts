import type { LinkPeer } from './voice-peer';
import type { IceCandidate, SessionDescription } from './voice-schemas';

type Applied = 'ignored' | 'resent' | 'answered' | 'accepted' | 'failed';

/** The perfect negotiation bookkeeping of one connection: whether an offer of
 *  ours is in flight, whether we are ignoring the remote offer that collided
 *  with it, and the last offer we answered so a repeat gets the same answer. */
export class Negotiation {
  private makingOffer = false;
  private ignoreOffer = false;
  private lastRemoteOffer = '';
  private lastAnswer: SessionDescription | null = null;

  reset(): void {
    this.makingOffer = false;
    this.ignoreOffer = false;
    this.lastRemoteOffer = '';
    this.lastAnswer = null;
  }

  get answer(): SessionDescription | null {
    return this.lastAnswer;
  }

  async makeOffer(peer: LinkPeer): Promise<SessionDescription | null> {
    this.makingOffer = true;
    try {
      await peer.setLocalDescription();
      return peer.localDescription;
    } catch {
      return null;
    } finally {
      this.makingOffer = false;
    }
  }

  async apply(peer: LinkPeer, description: SessionDescription, polite: boolean): Promise<Applied> {
    if (this.repeatsLastOffer(peer, description)) return 'resent';
    const collision =
      description.type === 'offer' && (this.makingOffer || peer.signalingState !== 'stable');
    this.ignoreOffer = !polite && collision;
    if (this.ignoreOffer) return 'ignored';
    try {
      await peer.setRemoteDescription(description);
      if (description.type === 'answer') return 'accepted';
      this.lastRemoteOffer = description.sdp;
      await peer.setLocalDescription();
      this.lastAnswer = peer.localDescription;
      return 'answered';
    } catch {
      return 'failed';
    }
  }

  async addCandidate(peer: LinkPeer, candidate: IceCandidate): Promise<void> {
    if (this.ignoreOffer) return;
    await peer.addIceCandidate(candidate).catch(() => undefined);
  }

  private repeatsLastOffer(peer: LinkPeer, description: SessionDescription): boolean {
    return (
      this.lastAnswer !== null &&
      description.type === 'offer' &&
      description.sdp === this.lastRemoteOffer &&
      peer.signalingState === 'stable'
    );
  }
}
