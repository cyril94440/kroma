import type { LinkPeer, PeerFactory } from './voice-peer';
import type { IceCandidate, SessionDescription } from './voice-schemas';

function describe(description: RTCSessionDescription | null): SessionDescription | null {
  if (description?.type !== 'offer' && description?.type !== 'answer') return null;
  return { type: description.type, sdp: description.sdp };
}

function candidateOf(candidate: RTCIceCandidate): IceCandidate {
  return {
    candidate: candidate.candidate,
    sdpMid: candidate.sdpMid,
    sdpMLineIndex: candidate.sdpMLineIndex,
    usernameFragment: candidate.usernameFragment,
  };
}

/** The browser's `RTCPeerConnection` behind the small surface the link uses.
 *  One send and receive audio transceiver exists from the start, so muting or
 *  swapping a microphone is a `replaceTrack`, never a renegotiation. */
export const createRtcPeer: PeerFactory = (handlers, iceServers) => {
  const connection = new RTCPeerConnection({ iceServers });
  const transceiver = connection.addTransceiver('audio', { direction: 'sendrecv' });
  const remote = new MediaStream();
  connection.onicecandidate = (event) => {
    if (event.candidate) handlers.onCandidate(candidateOf(event.candidate));
  };
  connection.onnegotiationneeded = () => handlers.onNegotiationNeeded();
  connection.onconnectionstatechange = () => handlers.onConnectionState(connection.connectionState);
  connection.ontrack = (event) => {
    remote.addTrack(event.track);
    handlers.onStream(remote);
  };
  const peer: LinkPeer = {
    get signalingState() {
      return connection.signalingState;
    },
    get connectionState() {
      return connection.connectionState;
    },
    get localDescription() {
      return describe(connection.localDescription);
    },
    setLocalDescription: () => connection.setLocalDescription(),
    setRemoteDescription: (description) => connection.setRemoteDescription(description),
    addIceCandidate: (candidate) => connection.addIceCandidate(candidate),
    restartIce: () => connection.restartIce(),
    setIceServers: (servers) =>
      connection.setConfiguration({ ...connection.getConfiguration(), iceServers: servers }),
    setTrack: (track) => void transceiver.sender.replaceTrack(track).catch(() => undefined),
    close: () => connection.close(),
  };
  return peer;
};
