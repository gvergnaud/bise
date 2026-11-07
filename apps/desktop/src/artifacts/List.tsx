// The artifacts' rows: the gallery's list view behind ☰ (ListHead, ListRow: page artifacts-gallery
// v7, art-grid; Grid.tsx draws them), and panel A's two-line rows for one project, grouped by day
// (amb-win's Panel.tsx mounts <ArtifactList project onOpen />). The list keys are app/listkeys.ts's
// (↑↓ jk pgup pgdn home end, ⏎ opens), the row's are rowKey's (space looks, v, c, r, @), the
// gallery's too. Owner: amb-mac (amb-kit's before); the list view art-grid.
import { memo, useState, type HTMLAttributes, type KeyboardEvent, type Ref } from 'react';
import { useListKeys } from '@/app/listkeys';
import { tipAttrs } from '@/ds';
import { byDay, glyphOf, keyOf, kindWords, madeBy, revealPathOf, splitArchived, timeOf, type Item } from './model';
import { isNew, lineTip, listKind, listName, topOf, whenOf, whoOf, type Card } from './gallery';
import { library } from '@/proto/words_library.gen';
import { onCardClick, type PickHow } from './Card';
import { Thumb } from './thumbs';
import { useAllArtifacts } from './hooks';
import { bridge, copyLink, openWithVersions, rowPrimary, usePutInMessage } from './Open';
import { leaves, LEAVES, useGoArtifact } from './leave';
import { openPr } from '@/review/openPr';
import './artifacts.css';
import { Press } from '@/ds';
import { panel } from '@/proto/words_panel.gen';

export interface RowsProps {
  items: readonly Item[];
  now: number;
  /** the selected row's key (project/id) */
  sel: string | null;
  onSel: (key: string) => void;
  onOpen: (it: Item) => void;
  /** space: a quick look (none: space opens) */
  onLook?: (it: Item) => void;
  /** @: put it in a message (none: @ does nothing) */
  onPut?: (it: Item) => void;
  testId: string;
}

/**
 * The selected row's keys of both forms, beside the list keys (app/listkeys.ts: ↑↓ jk pgup pgdn
 * home end ⏎), as the TUI's /artifacts (artifacts_screen.rs on_key): space looks, @ puts it in a
 * message, v opens it with its versions, c or ⌘C copies its link, r or o shows its file in the
 * Finder (a browser-only one: opens it). True when it took the key.
 */
export function rowKey(e: KeyboardEvent, cur: Item | undefined, p: Pick<RowsProps, 'onOpen' | 'onLook' | 'onPut'>): boolean {
  if (cur === undefined || e.altKey || e.ctrlKey) return false;
  if (e.metaKey) {
    // ⌘⏎: its head's first button, '↗ browser' else save a copy (front.ts, Open.tsx rowPrimary)
    if (e.key === 'Enter' && !e.shiftKey) {
      const run = rowPrimary(cur.a);
      if (run === null) return false;
      run();
      return true;
    }
    if (e.key !== 'c') return false;
    copyLink(cur.a.url);
    return true;
  }
  switch (e.key) {
    case ' ':
      (p.onLook ?? p.onOpen)(cur);
      return true;
    case '@':
      if (!p.onPut) return false;
      p.onPut(cur);
      return true;
    case 'v':
      openWithVersions(keyOf(cur));
      p.onOpen(cur);
      return true;
    case 'c':
      copyLink(cur.a.url);
      return true;
    case 'r':
    case 'o': {
      // the file, else bise's copy when it is gone (art-save)
      const path = revealPathOf(cur.a);
      const finder = bridge()?.showInFinder;
      if (path !== null && finder !== undefined) void finder(path);
      else if (leaves(cur.a)) p.onOpen(cur);
      return true;
    }
    default:
      return false;
  }
}

/**
 * Space on a row: Quick Look of its file when it has one and the window can (amb-mac's
 * window.bise.quickLook), else it opens in the window.
 */
export function useLook(open: (it: Item) => void): (it: Item) => void {
  return (it) => {
    const path = revealPathOf(it.a);
    const look = bridge()?.quickLook;
    if (path === null || look === undefined) {
      open(it);
      return;
    }
    void look(path).then((shown) => {
      if (!shown) open(it);
    });
  };
}

/** The rows grouped by day; the keys are its screen's (useListKeys + rowKey), a click picks a row. */
export function ArtifactRows({ items, now, sel, onSel, onOpen, testId }: RowsProps) {
  const groups = byDay(items, now);
  return (
    <div className="art-list panel" role="listbox" data-testid={testId}>
      {groups.map((g) => (
        <div key={g.day} className="art-day" data-testid={`${testId}.day.${g.day}`}>
          <div className="art-day-h">{g.title}</div>
          {g.items.map((it) => (
            <ArtRow key={keyOf(it)} it={it} on={sel === keyOf(it)} testId={`${testId}.row.${it.a.id}`}
              onPick={() => { onSel(keyOf(it)); }} onOpen={() => { onOpen(it); }} />
          ))}
        </div>
      ))}
    </div>
  );
}

const ArtRow = memo(function ArtRow({ it, on, onPick, onOpen, testId, ref }: {
  it: Item; on: boolean; onPick: () => void; onOpen: () => void; testId: string; ref?: Ref<HTMLDivElement>;
}) {
  const a = it.a;
  const title = (
    <span className="art-title">
      <span className="art-t">{a.title}</span>
      {a.new && <span className="art-new" data-testid={`${testId}.new`}>new</span>}
    </span>
  );
  // a browser-only one (a PR, a link): '↗' in place of its kind and version (designer m_21714)
  const out = leaves(a);
  const kind = out ? LEAVES : kindWords(a);
  return (
    <div ref={ref} className={`art-row${on ? ' on' : ''}`} role="option" aria-selected={on} data-testid={testId} data-row={keyOf(it)}
      data-leaves={out || undefined} onClick={onPick} onDoubleClick={onOpen}>
      <span className="art-g" aria-hidden="true">{glyphOf(a.kind)}</span>
      <span className="art-two">
        {title}
        <span className="art-sub">{`${kind} · ${madeBy(it, false)} · ${timeOf(a.at_ms)}`}</span>
      </span>
    </div>
  );
});

/** The gallery's list view's head (☰, page block b-pages 'list view'): name, made by, kind, when. */
export function ListHead() {
  return (
    <div className="art-lh" aria-hidden="true">
      <span />
      <span>{library.col_name}</span>
      <span>{library.col_made_by}</span>
      <span>{library.col_kind}</span>
      <span className="art-lw">{library.col_when}</span>
    </div>
  );
}

/**
 * One row of the gallery's list view (☰): a small picture (no marks on it), the name ('· 16
 * shots' for a stack) with its new chip, who made it, the kind in words (the list says it, the
 * cards never), when. A click opens as a card's, ⌘ or ⇧ click picks.
 */
export const ListRow = memo(function ListRow({ card, on, picked, now, visible, onPick, onOpen, onMenu, dragOf, testId }: {
  card: Card;
  on: boolean;
  picked: boolean;
  now: number;
  visible: boolean;
  onPick: (key: string, how: PickHow) => void;
  onOpen: (card: Card) => void;
  onMenu?: (key: string, at: { x: number; y: number }) => void;
  dragOf?: (key: string) => HTMLAttributes<HTMLElement>;
  testId: string;
}) {
  const it = topOf(card);
  const name = listName(card);
  return (
    <div {...dragOf?.(card.key)} className={`art-lr${on ? ' on' : ''}${picked ? ' picked' : ''}`} role="option" aria-selected={on || picked}
      data-row={card.key} data-testid={testId}
      onClick={onCardClick(card.key, onPick, () => { onOpen(card); })}
      onContextMenu={onMenu === undefined ? undefined : (e) => { e.preventDefault(); onMenu(card.key, { x: e.clientX, y: e.clientY }); }}>
      <span className="art-lr-th"><Thumb item={it} visible={visible} /></span>
      <span className="art-title">
        <span className="art-t" {...tipAttrs(name)}>{name}</span>
        {isNew(card) && <span className="art-new" data-testid={`${testId}.new`}>{panel.new_chip}</span>}
      </span>
      <span className="art-lr-a" {...tipAttrs(lineTip(card))}>{whoOf(it.a)}</span>
      <span className="art-lr-a">{listKind(card)}</span>
      <span className="art-lw">{whenOf(it.a.at_ms, now)}</span>
    </div>
  );
});

/**
 * Panel A's body (amb-win's frame): one project's artifacts, two-line rows by day. ⏎ opens one in
 * the window (onOpen with its id), space looks at its file (else opens), @ puts it in a message.
 */
export function ArtifactList({ project, onOpen, testId = 'artifacts.panel' }: { project: string; onOpen: (id: string) => void; testId?: string }) {
  const { live, archived } = splitArchived(useAllArtifacts(project));
  const put = usePutInMessage();
  // a browser-only one opens his browser (leave.ts), the others the frame's way
  const goArtifact = useGoArtifact();
  const openIt = (it: Item) => {
    // a PR: its own tab, as every PR door (review/openPr.ts)
    if (it.a.pr != null && it.a.url !== '') openPr(it.project, it.a.url);
    else if (leaves(it.a)) goArtifact(it.project, it.a.id, it.a);
    else onOpen(it.a.id);
  };
  const look = useLook(openIt);
  // the archived ones behind a faint 'N archived' row; a click shows them under the others
  const [showArchived, setShowArchived] = useState(false);
  // the clock of the day groups: when the panel mounted (a render reads no clock)
  const [now] = useState(() => Date.now());
  const items = showArchived ? [...live, ...archived] : live;
  const byKey = new Map(items.map((it) => [keyOf(it), it]));
  const put1 = (it: Item) => {
    put(it.project, it.a);
  };
  const { sel, setSel, ref, onKeyDown } = useListKeys<HTMLDivElement>({
    order: items.map(keyOf),
    onOpen: (k) => {
      const it = byKey.get(k);
      if (it) openIt(it);
    },
    onKey: (e, k) => rowKey(e, k === null ? undefined : byKey.get(k), { onOpen: openIt, onLook: look, onPut: put1 }),
  });
  const more =
    archived.length > 0 ? (
      <Press look="art-archived" onClick={() => { setShowArchived((o) => !o); }} data-testid={`${testId}.archived`}>
        {showArchived ? 'hide the archived ones' : panel.archived_count({ n: archived.length })}
      </Press>
    ) : null;
  if (items.length === 0) {
    return (
      <div className="art-empty" data-testid={`${testId}.empty`}>
        nothing yet. when an agent makes a page, a doc or a file for you, it lands here.
        {more}
      </div>
    );
  }
  return (
    <div ref={ref} tabIndex={-1} onKeyDown={onKeyDown} data-testid={`${testId}.keys`}>
      <ArtifactRows items={items} now={now} sel={sel} onSel={setSel} onOpen={openIt} onLook={look}
        onPut={put1} testId={testId} />
      {more}
    </div>
  );
}
