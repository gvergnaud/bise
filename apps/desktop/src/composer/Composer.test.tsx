// @vitest-environment happy-dom
// The composer mounted: its field is one node for its whole life (bug 1: never re-created by a
// prop change or a send), its words reach onSend, a command line runs, the draft survives a
// remount. Typing through the real key path is qa-type's (tests/checks/composer, Playwright).
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Composer } from './Composer';
import { askCaret } from './caret';
import type { ComposerActions } from './actions';
import type { Store } from './drafts';
import { NO_SOURCES } from './sources';
import { entryOf, entrySegments } from './entry';
import { buildMessage, type Segment } from './message';
import { composer as words } from '../proto/words_composer.gen';

afterEach(cleanup);

const memStore = (): Store => {
  const m = new Map<string, string>();
  return { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => void m.set(k, v), removeItem: (k) => void m.delete(k) };
};

async function mount(extra: Partial<Parameters<typeof Composer>[0]> = {}) {
  const got: { a: ComposerActions | null } = { a: null };
  const onSend = vi.fn();
  const store = extra.store ?? memStore();
  const ui = render(
    <Composer target="p/main" store={store} onSend={onSend} onReady={(a) => (got.a = a)} {...extra} />,
  );
  await act(async () => {
    await Promise.resolve();
  });
  if (got.a === null) throw new Error('composer not ready');
  return { ...ui, onSend, store, act: got.a };
}

describe('the composer', () => {
  it('asked for the caret (a chip just put in its draft): it takes the focus when it shows; never asked: it leaves it (ui-review m_52822)', async () => {
    const other = await mount({ target: 'p/other' });
    expect(other.getByTestId('composer.field').contains(document.activeElement)).toBe(false);
    cleanup();
    askCaret('p/main');
    const { getByTestId } = await mount();
    expect(getByTestId('composer.field').contains(document.activeElement)).toBe(true);
  });

  it('keeps one field node through prop changes and sends', async () => {
    const { getByTestId, rerender, onSend, act: a, store } = await mount();
    const field = getByTestId('composer.field');
    expect(field.getAttribute('contenteditable')).toBe('true');
    await act(async () => {
      a.insertText('ça va ?\nsecond line');
      await Promise.resolve();
    });
    rerender(<Composer target="p/main" store={store} onSend={onSend} busy head="to bise" />);
    expect(getByTestId('composer.field')).toBe(field);
    await act(async () => {
      a.submit();
      await Promise.resolve();
    });
    expect(onSend).toHaveBeenCalledWith({ text: 'ça va ?\nsecond line' });
    expect(getByTestId('composer.field')).toBe(field);
    expect(field.textContent).toBe('');
  });

  it("quote(text, from): a quote chip at the caret, then his words; the send carries it as a quote part at its place, with its number (amb-win, his m_22231)", async () => {
    const { getByTestId, onSend, act: a } = await mount();
    await act(async () => {
      a.quote('one\ntwo\nthree', 'main');
      a.insertText('why 03:00?');
      await Promise.resolve();
    });
    const chip = getByTestId('composer.chip.1');
    expect(chip.getAttribute('data-kind')).toBe('quote');
    await act(async () => {
      a.quote('   ', 'main');
      a.submit();
      await Promise.resolve();
    });
    const sent = onSend.mock.calls[0]?.[0] as { text: string; parts?: unknown[] } | undefined;
    // typed: the quote part where its chip was, then his words; the hub renders the model's tag (delta review D2)
    expect(sent?.parts).toEqual([{ kind: 'quote', from: 'main', text: 'one\ntwo\nthree', n: 1 }, { kind: 'text', text: ' why 03:00?' }]);
    expect(sent?.text).toBe('❝ 1 why 03:00?');
  });

  it('fill(text): the field holds the words alone, all selected, and nothing is sent (L1/M18, amb-kit)', async () => {
    const { getByTestId, onSend, act: a } = await mount();
    const field = getByTestId('composer.field');
    await act(async () => {
      a.insertText('an old draft');
      a.fill('show me what you can do');
      await Promise.resolve();
    });
    expect(field.textContent).toBe('show me what you can do');
    expect(onSend).not.toHaveBeenCalled();
    // selected: what he types next replaces it
    await act(async () => {
      a.insertText('hi');
      await Promise.resolve();
    });
    expect(field.textContent).toBe('hi');
    expect(getByTestId('composer.field')).toBe(field);
  });

  it("R18: esc no list took goes to the screen while busy or on a blank field (ProseMirror swallows esc's default); else, or when the screen did nothing, his words go to history", async () => {
    const onEscape = vi.fn();
    const esc = (el: Element) => {
      el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', keyCode: 27, bubbles: true, cancelable: true }));
    };
    const busy = await mount({ busy: true, onEscape });
    const field = busy.getByTestId('composer.field');
    await act(async () => {
      busy.act.insertText('hello');
      await Promise.resolve();
    });
    esc(field);
    // esc never stops a turn (ctrl+c does, main m_22205): the screen gets it first (a route hold);
    // it did nothing here, so his words go to history as at rest
    expect(onEscape, 'busy: the screen first').toHaveBeenCalledTimes(1);
    expect(field.textContent, 'the screen did nothing: put away').toBe('');
    busy.unmount();

    const idle = await mount({ onEscape, target: 'p/other' });
    const f2 = idle.getByTestId('composer.field');
    esc(f2);
    expect(onEscape, 'blank: the screen (keep with bise, ...)').toHaveBeenCalledTimes(2);
    await act(async () => {
      idle.act.insertText('later');
      await Promise.resolve();
    });
    esc(f2);
    expect(onEscape, 'idle with words: put away, not the screen').toHaveBeenCalledTimes(2);
    expect(f2.textContent).toBe('');
  });

  it('runs a typed command line instead of sending it', async () => {
    const onCommand = vi.fn();
    const sources = { ...NO_SOURCES, commands: [{ name: '/model', desc: 'set the model', args: true, ready: true }] };
    const { onSend, act: a } = await mount({ sources, onCommand });
    await act(async () => {
      a.insertText('/model opus');
      a.submit();
      await Promise.resolve();
    });
    expect(onCommand).toHaveBeenCalledWith('/model', 'opus');
    expect(onSend).not.toHaveBeenCalled();
  });

  it('colors a code block as he types, keeps the caret and the text (B9/W8)', async () => {
    const { getByTestId, onSend, act: a } = await mount();
    const field = getByTestId('composer.field');
    await act(async () => {
      a.insertText('- a\n```rust\nfn main');
      await Promise.resolve();
    });
    const styleOf = (word: string): string | null =>
      Array.from(field.querySelectorAll('span')).find((s) => s.textContent === word)?.getAttribute('style') ?? null;
    // the bubble's colors (thread/Code.tsx codeLines, code-fences)
    expect(styleOf('fn')).toContain('--md-syn-keyword');
    expect(styleOf(' ')).toBeNull();
    // the caret stayed at the end through the redraw: more words land after 'main'
    await act(async () => {
      a.insertText('() {}\n```');
      await Promise.resolve();
    });
    await act(async () => {
      a.submit();
      await Promise.resolve();
    });
    expect(onSend).toHaveBeenCalledWith({ text: '- a\n```rust\nfn main() {}\n```' });
  });

  it('keeps the draft of each target across a remount', async () => {
    const store = memStore();
    const first = await mount({ store });
    await act(async () => {
      first.act.insertText('half a thought');
      await Promise.resolve();
    });
    first.unmount();
    const again = await mount({ store });
    expect(again.getByTestId('composer.field').textContent).toBe('half a thought');
    again.unmount();
    const other = await mount({ store, target: 'p/other' });
    expect(other.getByTestId('composer.field').textContent).toBe('');
  });

  it('reads the entries it saved ({v:2,segs}): his draft, history and queue come back whole (lead m_11680)', async () => {
    const segs: Segment[] = [
      { kind: 'text', text: 'café 👋 déjà\nligne 2 ' },
      { kind: 'chip', chip: { kind: 'paste', n: 1, text: 'a\nb' } },
      { kind: 'text', text: ' fin' },
    ];
    const v2 = entryOf(segs);
    expect(JSON.parse(v2)).toMatchObject({ v: 2 });
    // each place an entry is read: the draft, a history line (↑), a queued message
    expect(entrySegments(v2)).toEqual(segs);
    const store = memStore();
    const queued = entryOf([{ kind: 'text', text: 'après ça 🎉' }]);
    store.setItem('bise.composer.v1.p/main', JSON.stringify({ draft: v2, history: [v2], queue: [queued] }));
    const onSend = vi.fn();
    const onQueue = vi.fn();
    const got: { a: ComposerActions | null } = { a: null };
    const el = (busy: boolean) => <Composer target="p/main" store={store} busy={busy} onSend={onSend} onQueue={onQueue} onReady={(a) => (got.a = a)} />;
    const ui = render(el(true));
    await act(async () => {
      await Promise.resolve();
    });
    expect(ui.getByTestId('composer.field').textContent).toContain('café 👋 déjà');
    expect(ui.getByTestId('composer.field').textContent).toContain('ligne 2');
    expect(ui.getByTestId('composer.field').textContent).toContain('fin');
    await act(async () => {
      ui.rerender(el(false)); // the turn ends: the window sends nothing itself (the hub does)
      await Promise.resolve();
    });
    expect(onSend).not.toHaveBeenCalled();
    await act(async () => {
      got.a?.submit(); // the draft sends exactly what the TipTap engine would have sent
      await Promise.resolve();
    });
    expect(onSend).toHaveBeenLastCalledWith(buildMessage(segs));
  });

  // min-tests: composer/queue-steer: tab during a turn queues his words and sends nothing; ⏎ sends at once
  it('tab during a turn queues his words, the field empties, nothing sent; ⏎ during a turn sends at once', async () => {
    const onQueue = vi.fn();
    const { getByTestId, onSend, act: a } = await mount({ busy: true, onQueue });
    const field = getByTestId('composer.field');
    await act(async () => {
      a.insertText('after this one');
      await Promise.resolve();
    });
    fireEvent.keyDown(field, { key: 'Tab', code: 'Tab' });
    expect(onQueue).toHaveBeenCalledWith(buildMessage([{ kind: 'text', text: 'after this one' }]));
    expect(onSend).not.toHaveBeenCalled();
    expect(field.textContent).toBe('');
    await act(async () => {
      a.insertText('now');
      a.submit();
      await Promise.resolve();
    });
    expect(onSend).toHaveBeenLastCalledWith(buildMessage([{ kind: 'text', text: 'now' }]));
  });

  // min-tests: composer/approvals-toggle (TP-R6) and composer/no-undo (bar T K23)
  it('⇧⇥ in the field is the approvals switch (onShiftTab), never an outdent; ⌘Z with nothing to undo says so', async () => {
    const onShiftTab = vi.fn();
    const { getByTestId, container, act: a } = await mount({ onShiftTab });
    const field = getByTestId('composer.field');
    await act(async () => {
      a.insertText('- an item');
      await Promise.resolve();
    });
    fireEvent.keyDown(field, { key: 'Tab', code: 'Tab', shiftKey: true });
    expect(onShiftTab).toHaveBeenCalledTimes(1);
    expect(field.textContent).toBe('- an item');
    expect(container.textContent).not.toContain(words.no_undo);
    await act(async () => {
      a.submit();
      await Promise.resolve();
    });
    await act(async () => {
      fireEvent.keyDown(field, { key: 'z', code: 'KeyZ', metaKey: true });
      await Promise.resolve();
    });
    expect(container.textContent).toContain(words.no_undo);
  });

  // r2-hints (his m_44255, designer m_44287): the held keys came back after nm-composer's key bar left
  it('a modifier held alone: ⌃ puts ⌃R on the mic in place of its icon, and the row lists its keys; nothing held, neither', async () => {
    const dictate = { start: () => 'd1', stop: () => {}, cancel: () => {} } as unknown as NonNullable<typeof NO_SOURCES.dictate>;
    const sources = { ...NO_SOURCES, dictate, voice: () => {} };
    const ctx = { fold: null, inbox: 2, itemOpen: false, code: false, agents: 0, thread: true };
    const { getByTestId, queryByTestId, rerender, onSend, store } = await mount({ sources });
    expect(queryByTestId('composer.held')).toBeNull();
    expect(queryByTestId('composer.mic.hint')).toBeNull();
    rerender(<Composer target="p/main" store={store} onSend={onSend} sources={sources} busy held={{ mod: 'Control', ctx }} />);
    expect(getByTestId('composer.mic.hint').textContent).toBe('⌃R');
    expect(getByTestId('composer.mic').querySelector('svg')).toBeNull();
    const pairs = [...getByTestId('composer.held').querySelectorAll('[data-testid="composer.held.pair"]')].map((p) => p.textContent);
    // ⌃R is on the mic, never in the list too (designer m_44571)
    expect(pairs).toEqual([`⌃C${words.bar_stop}`, `⌃1…⌃2${words.held_open_item}`, `⌃R⌃R${words.bar_voice_mode}`, `⌃\`${words.bar_terminal}`]);
    // ⌘ held: its own keys, the mic keeps its icon
    rerender(<Composer target="p/main" store={store} onSend={onSend} sources={sources} held={{ mod: 'Meta', ctx }} />);
    expect(queryByTestId('composer.mic.hint')).toBeNull();
    expect(getByTestId('composer.held').dataset.mod).toBe('Meta');
    expect(getByTestId('composer.held').textContent).toContain(words.held_new_project);
    rerender(<Composer target="p/main" store={store} onSend={onSend} sources={sources} held={null} />);
    expect(queryByTestId('composer.held')).toBeNull();
  });
});
