import type { Send, SendResult } from './voice-peer';
import type { SignalBody } from './voice-schemas';
import { LINK_TIMING } from './voice-timing';

/** Sends one connection's signals strictly one after the other. Each is an
 *  independent HTTP request, and two in flight together can reach the server in
 *  either order, which would hand a candidate to the other side before the
 *  description it belongs to. A failed request is retried a couple of times; one
 *  that belongs to a session already replaced is not sent at all. */
export class Outbox {
  private tail: Promise<unknown> = Promise.resolve();
  private open = true;

  constructor(
    private readonly send: Send,
    private readonly isStale: (body: SignalBody) => boolean,
  ) {}

  push(body: SignalBody): Promise<SendResult> {
    const result = this.tail.then(() => this.deliver(body));
    this.tail = result.catch(() => undefined);
    return result;
  }

  close(): void {
    this.open = false;
  }

  private async deliver(body: SignalBody): Promise<SendResult> {
    let result: SendResult = 'failed';
    for (let attempt = 0; attempt <= LINK_TIMING.sendRetryMs.length; attempt += 1) {
      if (!this.open || this.isStale(body)) return 'failed';
      result = await this.send(body);
      if (result !== 'failed') return result;
      const wait = LINK_TIMING.sendRetryMs[attempt];
      if (wait !== undefined) await new Promise((done) => setTimeout(done, wait));
    }
    return result;
  }
}
