// @vitest-environment happy-dom
// at-menu (his m_66958): names of artifacts and files often hold spaces, and the @ menu closed on
// the first one. Designer m_67057's rule: once open, the menu runs past spaces and keeps
// filtering; it closes on Esc, on a pick (the whole '@my file na' becomes one reference), or when
// nothing matches any more (then his words stay plain and never reopen it).
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { ComposerActions } from './actions';
import type { Store } from './drafts';
import type { Message } from './message';
import { NO_SOURCES, type ComposerSources } from './sources';
import { PmField } from './pm/PmField';

afterEach(cleanup);

const memStore = (): Store => {
  const m = new Map<string, string>();
  return { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => void m.set(k, v), removeItem: (k) => void m.delete(k) };
};

const artifact = (id: string, title: string) => ({ id, title, kind: 'page', agent: 'designer', url: `https://x/${id}`, atMs: 0 });

const SOURCES: ComposerSources = {
  ...NO_SOURCES,
  agents: [{ name: 'main' }],
  artifacts: [artifact('a1', 'my file name'), artifact('a2', 'my notes'), artifact('a3', 'main loop notes')],
  files: () => Promise.resolve([]),
};

function field() {
  const got: { a: ComposerActions | null; sent: Message[] } = { a: null, sent: [] };
  const r = render(
    <div className="box">
      <PmField target="p/main" busy={false} sources={SOURCES} store={memStore()} testId="composer" placeholder="write to bise"
        send={(m) => void got.sent.push(m)} queue={() => undefined} editNewest={() => false} command={() => undefined}
        onCanSend={() => undefined} onStrip={() => undefined} onBar={() => undefined} onActions={(x) => (got.a = x)} />
    </div>,
  );
  const settle = async () => {
    for (let i = 0; i < 4; i += 1) await Promise.resolve();
  };
  /** his keys, one character at a time, as he types them */
  const type = async (text: string) => {
    for (const ch of text)
      await act(async () => {
        got.a?.insertText(ch);
        await settle();
      });
  };
  const key = async (k: string) => {
    await act(async () => {
      r.getByTestId('composer.field').dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
      await settle();
    });
  };
  const rows = () => [...(r.queryByTestId('composer.pick')?.querySelectorAll('[data-row]') ?? [])].map((e) => e.textContent ?? '');
  return { got, r, type, key, rows, text: () => r.getByTestId('composer.field').textContent };
}

describe('the @ menu across spaces', () => {
  it("'@my file na' keeps the menu open and filters on the whole run, spaces included", async () => {
    const f = field();
    await f.type('look at @my');
    expect(f.rows().filter((t) => t.includes('my '))).toHaveLength(2);
    await f.type(' file na');
    expect(f.rows().some((t) => t.includes('my file name')), 'the space kept it open').toBe(true);
    expect(f.rows().some((t) => t.includes('my notes')), 'and it filters past the space').toBe(false);
    // designer m_67057: the run it filters on, on a faint ground, from the @ to the caret
    expect(f.r.container.querySelector('[data-at-run]')?.textContent).toBe('@my file na');
  });

  it('esc closes it, the words stay, and the next space does not reopen it', async () => {
    const f = field();
    await f.type('@my fi');
    expect(f.rows().length).toBeGreaterThan(0);
    await f.key('Escape');
    expect(f.r.queryByTestId('composer.pick')).toBeNull();
    expect(f.r.container.querySelector('[data-at-run]'), 'the ground goes with the menu').toBeNull();
    await f.type('le na');
    expect(f.r.queryByTestId('composer.pick')).toBeNull();
    expect(f.text()).toBe('@my file na');
  });

  it("a pick turns the whole '@my file na' into one reference, sent as one link", async () => {
    const f = field();
    await f.type('see @my file na');
    await f.key('Enter');
    expect(f.r.queryByTestId('composer.pick')).toBeNull();
    expect(f.text()).not.toContain('@my');
    await act(async () => {
      f.got.a?.submit();
      await Promise.resolve();
    });
    const text = JSON.stringify(f.got.sent[0]);
    expect(text).toContain('[my file name](https://x/a1)');
    expect(text).not.toContain('@my');
    expect(text.split('https://x/a1').length - 1).toBe(1);
  });

  it("a plain sentence after an unmatched @ is not swallowed: '@xyz is great' stays text, no menu", async () => {
    const f = field();
    await f.type('@xyz is great');
    expect(f.r.queryByTestId('composer.pick')).toBeNull();
    expect(f.text()).toBe('@xyz is great');
    await f.type('. thanks @main');
    expect(f.rows().some((t) => t.includes('main')), 'a new @ opens it again').toBe(true);
  });

  it("an agent pick ends it: '@main loop' never reopens the menu, even on words an artifact holds", async () => {
    const f = field();
    await f.type('@ma');
    await f.key('Enter');
    expect(f.text()).toBe('@main ');
    expect(f.r.queryByTestId('composer.pick')).toBeNull();
    await f.type('loop');
    expect(f.r.queryByTestId('composer.pick')).toBeNull();
  });
});
