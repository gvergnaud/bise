// @vitest-environment happy-dom
// The panel full width (panel-full, his m_45439), its law: past the threshold the thread's rows are
// gone and the floating composer owns his words (the same draft, the same caret, the focus); back
// under it, the thread's docked composer comes back with the draft intact. The real thread screen,
// the real floating composer, the real field (ProseMirror), the screen's gate App.tsx uses.
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { Editor } from '@tiptap/core';
import type { CorePort } from '@/shared/api';
import { replay, Store, StoreProvider } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { ThreadScreen } from '@/thread/ThreadScreen';
import { FloatHost } from '@/composer/float/Host';
import { resetHandoff } from '@/composer/handoff';
import { UnlessFull } from './Full';
import { FOLDED, openIn, FIRST } from './model';
import { EMPTY_REC, putView, switchTo } from './scope';
import { panelState, setFull, toggleFull } from './state';

const P = 'acme-1a2b3c4d';

beforeEach(() => {
  // wide enough for the panel beside the thread (under 1100 it is a sheet, never full)
  Object.defineProperty(window, 'innerWidth', { value: 1440, configurable: true });
  localStorage.clear();
  resetHandoff();
  // open on a tab: an open panel always shows one (model.ts withOpen)
  panelState.reset(putView(switchTo(EMPTY_REC, P), { ...FOLDED, open: true, tabs: openIn(FIRST, { kind: 'browser', n: 1 }) }));
});
afterEach(cleanup);

function mount() {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  replay(store);
  return render(
    <StoreProvider store={store}>
      <RouterContext value={createRouter({ screen: 'thread', project: P, agent: 'perf' })}>
        <UnlessFull>
          <ThreadScreen project={P} agent="perf" />
        </UnlessFull>
        <FloatHost />
      </RouterContext>
    </StoreProvider>,
  );
}

/** The one field on screen and its editor (TipTap keeps it on its node). */
const field = (v: ReturnType<typeof mount>) => {
  const all = v.getAllByTestId('composer.field');
  expect(all, 'one composer at a time').toHaveLength(1);
  const el = all[0] as HTMLElement & { editor?: Editor };
  if (el.editor === undefined) throw new Error('no editor on the field');
  return { el, ed: el.editor };
};
const settle = () => act(async () => {
  await Promise.resolve();
});

describe('the panel full width', () => {
  it('the float owns his words in full width (same draft, same caret, the focus), the docked composer has them back after', async () => {
    const v = mount();
    await settle();
    expect(v.queryByTestId('thread.screen')).not.toBeNull();
    expect(v.queryByTestId('composer.float'), 'no float on a thread beside the panel').toBeNull();
    const docked = field(v);
    act(() => {
      docked.ed.commands.insertContent('half a thought');
      docked.ed.commands.setTextSelection(5);
      docked.ed.view.focus();
    });

    act(() => {
      setFull(true);
    });
    await settle();
    expect(v.queryByTestId('thread.screen'), 'the thread rows are gone').toBeNull();
    const float = v.getByTestId('composer.float');
    expect(float.dataset['agent']).toBe('perf');
    const f = field(v);
    expect(float.contains(f.el)).toBe(true);
    expect(f.el.textContent).toBe('half a thought');
    expect(f.ed.state.selection.from).toBe(5);
    expect(f.ed.view.hasFocus()).toBe(true);

    // he types on in the float, the caret moves
    act(() => {
      f.ed.commands.insertContent(', more');
    });
    act(() => {
      toggleFull();
    });
    await settle();
    expect(v.queryByTestId('composer.float')).toBeNull();
    expect(v.queryByTestId('thread.screen')).not.toBeNull();
    const back = field(v);
    expect(back.el.textContent).toBe('half, more a thought');
    expect(back.ed.state.selection.from).toBe(11);
  });

  it('a composer that goes for any other reason leaves no caret: the next one starts at its end', async () => {
    const v = mount();
    await settle();
    const docked = field(v);
    act(() => {
      docked.ed.commands.insertContent('half a thought');
      docked.ed.commands.setTextSelection(5);
    });
    v.unmount();
    const again = mount();
    await settle();
    const f = field(again);
    expect(f.el.textContent).toBe('half a thought');
    expect(f.ed.state.selection.from).toBe(f.ed.state.doc.content.size - 1);
  });
});
