// The run of an open `@` menu's query, from the `@` to the caret, on a faint --c-hover ground
// (at-menu, designer m_67057): its query runs past spaces now, so he sees which words it filters
// on. An inline decoration, never a mark (the document, the send and the undo history stay as
// typed); gone the moment the menu closes. The TUI underlines the same run.
import { Extension } from '@tiptap/core';
import { Plugin, PluginKey, type EditorState, type Transaction } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';

export const AT_RUN_STYLE = 'background: var(--c-hover); border-radius: 2px';

/** [from, to) document positions, or null: no menu open. */
type Run = readonly [number, number] | null;

interface RunState {
  readonly run: Run;
  readonly set: DecorationSet;
}

const KEY = new PluginKey<RunState>('bise-at-run');

const same = (a: Run, b: Run): boolean => a === b || (a !== null && b !== null && a[0] === b[0] && a[1] === b[1]);

/** The transaction that draws the open menu's run (null: none); null when it is drawn already. */
export function setAtRun(state: EditorState, run: Run): Transaction | null {
  const now = KEY.getState(state)?.run ?? null;
  const ok = run === null || (run[0] < run[1] && run[0] >= 0 && run[1] <= state.doc.content.size);
  const want = ok ? run : null;
  if (same(now, want)) return null;
  return state.tr.setMeta(KEY, want).setMeta('addToHistory', false);
}

export const AtRun = Extension.create({
  name: 'biseAtRun',
  addProseMirrorPlugins: () => [
    new Plugin<RunState>({
      key: KEY,
      state: {
        init: () => ({ run: null, set: DecorationSet.empty }),
        apply: (tr, old, _before, state) => {
          const meta = tr.getMeta(KEY) as Run | undefined;
          if (meta === undefined) return tr.docChanged ? { run: old.run, set: old.set.map(tr.mapping, state.doc) } : old;
          return {
            run: meta,
            set: meta === null ? DecorationSet.empty : DecorationSet.create(state.doc, [Decoration.inline(meta[0], meta[1], { style: AT_RUN_STYLE, 'data-at-run': 'true' })]),
          };
        },
      },
      props: {
        decorations: (state) => KEY.getState(state)?.set,
      },
    }),
  ],
});
