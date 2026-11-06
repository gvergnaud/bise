import { describe, expect, it } from 'vitest';
import { answeredEv, diffRefusedEv, lateAnsweredEv, refusedEv, RESTARTED_WORDS, sendRefusedEv } from './local';

const P = 'acme-1a2b3c4d';
const pick = { agent: 'perf', notice: 'you answered perf: warm', pick: 'warm', question: 'which bench?' };

describe("the window-local events main builds from the core's answers (ipc.ts, the nightly check)", () => {
  it('an answer: card_answered with the hub\'s answer, or a null answer when the step closed none', () => {
    expect(answeredEv({ cmd: 'answer', project: P, card: 9, reply: '2' }, { answer: pick })).toEqual({ ev: 'card_answered', project: P, card: 9, answer: pick });
    expect(answeredEv({ cmd: 'answer', project: P, card: 9, reply: '2' }, {})).toEqual({ ev: 'card_answered', project: P, card: 9, answer: null });
    expect(answeredEv({ cmd: 'answer', project: P, card: 9, reply: '2' }, null)).toEqual({ ev: 'card_answered', project: P, card: 9, answer: null });
  });

  it("a refused diff: the hub's typed error with its cmd tag; any other command: none", () => {
    expect(diffRefusedEv({ cmd: 'diff', project: P, agent: 'perf' }, { message: 'no agent perf' })).toEqual({ ev: 'error', project: P, cmd: 'diff', text: 'no agent perf' });
    expect(diffRefusedEv({ cmd: 'send', project: P }, { message: 'x' })).toBeNull();
  });

  it('a send: sent with its message id and why none was made, both null from an older hub', () => {
    const send = { cmd: 'send', project: P, agent: 'main', text: 'hi', cid: 3 };
    expect(answeredEv(send, { msg_id: 41, reason: 'made' })).toEqual({ ev: 'sent', project: P, agent: 'main', cid: 3, msg_id: 41, reason: 'made' });
    expect(answeredEv(send, { reason: 'routed' })).toEqual({ ev: 'sent', project: P, agent: 'main', cid: 3, msg_id: null, reason: 'routed' });
    expect(answeredEv(send, {})).toEqual({ ev: 'sent', project: P, agent: 'main', cid: 3, msg_id: null, reason: null });
  });

  it('a close taken, or any other command: no event (the rows say it)', () => {
    expect(answeredEv({ cmd: 'close', project: P, card: 9 }, {})).toBeNull();
    expect(answeredEv({ cmd: 'subscribe', project: P, agent: 'main' }, {})).toBeNull();
    expect(answeredEv({ cmd: 'shown', projects: [P] }, {})).toBeNull();
  });

  it("a refusal: card_refused with the core's words; a core gone before it answered says so", () => {
    expect(refusedEv(P, 9, "notification #9 isn't open")).toEqual({ ev: 'card_refused', project: P, card: 9, text: "notification #9 isn't open" });
    expect(refusedEv(P, 9, RESTARTED_WORDS)).toEqual({ ev: 'card_refused', project: P, card: 9, text: RESTARTED_WORDS });
    expect(RESTARTED_WORDS).toMatch(/^bise restarted before your answer was confirmed/);
  });

  it("a send the core answered with an error: send_refused, its reason typed from data, never from the words", () => {
    const send = { cmd: 'send', project: P, agent: 'main', text: 'hi', cid: 3 };
    const at = (reason: unknown) => sendRefusedEv(send, { data: { reason } });
    expect(at('late')).toEqual({ ev: 'send_refused', project: P, agent: 'main', cid: 3, reason: 'late' });
    expect(at('gone')).toMatchObject({ reason: 'gone' });
    expect(at('undelivered')).toMatchObject({ reason: 'refused' });
    expect(sendRefusedEv(send, {})).toMatchObject({ reason: 'refused' });
    expect(sendRefusedEv({ cmd: 'answer', project: P, card: 9 }, { data: { reason: 'late' } })).toBeNull();
  });

  it("the core's send_answered: the window's sent with the hub's late result, or send_refused gone", () => {
    expect(lateAnsweredEv({ ev: 'send_answered', project: P, agent: 'main', cid: 3, result: { msg_id: 41, reason: 'made' } })).toEqual({ ev: 'sent', project: P, agent: 'main', cid: 3, msg_id: 41, reason: 'made' });
    expect(lateAnsweredEv({ ev: 'send_answered', project: P, agent: 'main', cid: 3 })).toEqual({ ev: 'send_refused', project: P, agent: 'main', cid: 3, reason: 'gone' });
    expect(lateAnsweredEv({ ev: 'sent', text: 'x' })).toBeNull();
  });
});
