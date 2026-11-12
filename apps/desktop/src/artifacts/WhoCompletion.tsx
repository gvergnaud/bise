// The search field's completion (designer m_61790): while a `by:` token is typed, the ds PickerPop
// under the field lists people first (you), then the agents with the most artifacts in the scope
// (filters.ts whoOptions); ↑↓ ⏎ pick, esc closes, a pick writes the token in the field. `since:`
// lists the When values. The rule is filters.ts's; this is its drawing. Owner: artifacts-keep.
import type { RefObject } from 'react';
import { PickerPop, type PickerRow } from '@/ds';
import { library } from '@/proto/words_library.gen';
import { appOptions, completeAt, tokenAt, whenFacet, whoOptions, type Ctx, type Filters, type When } from './filters';
import type { Item } from './model';
import { WHEN_WORD } from './Toolbar';

/** The completion open at the caret: the token typed, where it starts, its rows. */
export interface Completion {
  readonly key: string;
  readonly start: number;
  readonly caret: number;
  readonly rows: readonly PickerRow[];
}

/** What the field completes at `caret` (the Screen reads it from the field's change), none: closed. Pure. */
export function completionOf(q: string, caret: number | null, items: readonly Item[], filters: Filters, ctx: Ctx): Completion | null {
  const t = caret === null ? null : tokenAt(q, caret);
  if (t === null || caret === null) return null;
  const rows: PickerRow[] = t.key === 'by'
    ? whoOptions(items, filters, ctx, t.typed).map((o) => ({ id: o.key, name: o.key === 'you' ? library.by_you : o.key, trailing: String(o.n), testId: `artifacts.complete.${o.key}` }))
    : t.key === 'since'
      ? whenFacet(items, filters, ctx).filter((o) => o.key.startsWith(t.typed.toLowerCase())).map((o) => ({ id: o.key, name: WHEN_WORD[o.key as When], trailing: String(o.n), disabled: o.n === 0, testId: `artifacts.complete.${o.key}` }))
      : t.key === 'app'
        // the app: token (apps-gallery, plugin-apps): the apps in scope, the most first
        ? appOptions(items, filters, ctx, t.typed).map((o) => ({ id: o.key, name: o.word, trailing: String(o.n), testId: `artifacts.complete.app.${o.key}` }))
        : [];
  return rows.length === 0 ? null : { key: t.key, start: t.start, caret, rows };
}

/** The text once `id` is picked: the token written whole, a space after. Pure. */
export const pickedText = (q: string, c: Completion, id: string): string => completeAt(q, c.start, c.caret, c.key, id);

/** The active row: the one he walked to while it is still listed, else the first that can be picked. Pure. */
export function activeRow(c: Completion, walked: string | null): string | null {
  const ids = c.rows.filter((r) => r.disabled !== true).map((r) => r.id);
  return walked !== null && ids.includes(walked) ? walked : (ids[0] ?? null);
}

/** The row ↓ (1) or ↑ (-1) walks to from `at`, wrapping. Pure. */
export function walk(c: Completion, at: string | null, by: 1 | -1): string | null {
  const ids = c.rows.filter((r) => r.disabled !== true).map((r) => r.id);
  if (ids.length === 0) return null;
  const i = at === null ? -1 : ids.indexOf(at);
  return ids[(i + by + ids.length) % ids.length] ?? null;
}

/** Under the field, its keys left to the field (he keeps typing; ↑↓ ⏎ esc are the Screen's). */
export function WhoCompletion({ c, active, field, onPick, onClose }: { readonly c: Completion; readonly active: string | null; readonly field: RefObject<HTMLInputElement | null>; readonly onPick: (id: string) => void; readonly onClose: () => void }) {
  return (
    <PickerPop anchor={field} align="start" testId="artifacts.complete" callerKeys active={active}
      groups={[{ id: c.key, head: c.key === 'by' ? library.by_head : c.key === 'app' ? library.apps_label : library.when, rows: c.rows }]}
      onPick={onPick} onClose={onClose} />
  );
}
