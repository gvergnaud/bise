// The web host's pure parts (C9): a view command as POST /api/view's body (C5's method, no
// window ids), the page's answer read as the store's, its quiet words.
import { describe, expect, it } from 'vitest';
import { views } from '@/proto/words_views.gen';
import { answerOf, bodyOf, wordsOf } from './web';

describe('the web host', () => {
  it("a command is C5's method and params, without the window's project", () => {
    expect(bodyOf({ cmd: 'view_resource', project: '', agent: 'a', call: '1-1' })).toEqual({ method: 'view/resource', params: { agent: 'a', call: '1-1' } });
    expect(bodyOf({ cmd: 'view_call', project: '', agent: 'a', call: '1-1', method: 'tools/call', params: { name: 'bump' } })).toEqual({ method: 'view/call', params: { agent: 'a', call: '1-1', method: 'tools/call', params: { name: 'bump' } } });
    expect(bodyOf({ cmd: 'view_message', project: '', agent: 'a', call: '1-1', content: [{ type: 'text', text: 'hi' }] })).toEqual({ method: 'view/message', params: { agent: 'a', call: '1-1', content: [{ type: 'text', text: 'hi' }] } });
  });

  it('reads the answer: a result, an error as given, anything else an error', () => {
    expect(answerOf(true, { result: { ok: 1 } })).toEqual({ result: { ok: 1 } });
    expect(answerOf(true, { error: { code: -32037, message: 'no gesture', data: { reason: 'no-gesture' } } })).toEqual({ error: { code: -32037, message: 'no gesture', data: { reason: 'no-gesture' } } });
    expect(answerOf(false, null)).toMatchObject({ error: { code: -32603 } });
    expect(answerOf(true, {})).toMatchObject({ error: { code: -32603 } });
  });

  it('says loading until the view shows, then nothing', () => {
    expect(wordsOf({ kind: 'loading', height: 160 })).toBe(views.loading);
    expect(wordsOf({ kind: 'expired' })).toBe(views.expired);
    expect(wordsOf({ kind: 'failed', message: 'x' })).toBe(views.failed);
  });
});
