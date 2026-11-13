// src/app/panel · the omnibox ⌘T's New tab and ⌘L's blank browser share (page newtab-omnibox-flow,
// designer m_64584): the hook that ranks his web pages and artifacts for his words (omni.ts) and
// keeps the field's state (omnifield.ts), the field's text and its inline part selected, its keys,
// what a row opens (a page or a search in his browser, noted as typed: ⏎ brings it to the front
// and closes the tab, ⌘⏎ leaves it behind and keeps the tab; an artifact in this tab, or with ⌘⏎
// in a new tab behind this one), and the list's rows (a page's × takes it out of his history).
// Owner: newtab-omnibox.
import { useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode, type RefObject } from 'react';
import { AppIcon, IconButton, KindIcon, iconKindOf } from '@/ds';
import { panel } from '@/proto/words_panel.gen';
import { useAllArtifacts } from '@/artifacts';
import { sayOpened } from '@/artifacts/opened';
import { getAppStore } from '@/store';
import { activateTab, closeTabId, openWhat, usePanelState } from './state';
import { hostOf, omniHits, omniRows, shownUrl, type OmniHit, type OmniRow } from './omni';
import { clampAt, onInput, onKey, shownOf, type OmniField } from './omnifield';
import { removeWebPage, useWebHistory } from './webpages';
import { makeKey, type MakeOpt } from './make';
import { PickRow } from './Launcher';

/** The omnibox of the tab `tabId`: his words (kept by the caller), the list, the field's text. */
export interface Omni {
  readonly field: OmniField;
  readonly hits: readonly OmniHit[];
  readonly rows: readonly OmniRow[];
  /** the field's text */
  readonly value: string;
  /** the field's onChange */
  readonly input: (value: string, how: InputEvent | undefined) => void;
  /** the field's onKeyDown: true when it took the key (the caller leaves it alone) */
  readonly key: (e: KeyboardEvent<HTMLInputElement>) => boolean;
  /** opens a row (`behind`: ⌘⏎, ⌘click) */
  readonly open: (r: OmniRow, behind: boolean) => void;
  /** a pointer over a row takes the highlight (the field's text stays) */
  readonly point: (i: number) => void;
}

/** The artifacts in Recent, 'project/id' (one more visit of today for the ranking). */
function useRecentArts(): ReadonlySet<string> {
  const recent = usePanelState().recent;
  return useMemo(() => new Set(recent.flatMap((e) => (e.kind === 'artifact' ? [`${e.project}/${e.id}`] : []))), [recent]);
}

/**
 * The omnibox over the tab `tabId`'s field `ref`: `typed` and `setTyped` his words (the launcher
 * keeps them as its draft), `makes` New ›'s rows his words found, `make` what picking one does.
 */
export function useOmni(o: {
  readonly tabId: string;
  readonly ref: RefObject<HTMLInputElement | null>;
  readonly typed: string;
  readonly setTyped: (q: string) => void;
  readonly makes?: readonly MakeOpt[];
  readonly make?: (m: MakeOpt) => void;
}): Omni {
  const arts = useAllArtifacts();
  const web = useWebHistory();
  const recent = useRecentArts();
  const [now] = useState(() => Date.now());
  // the rest of the field's state; his words are the caller's (a draft that comes back with the tab)
  const [st, setSt] = useState<Omit<OmniField, 'typed'>>({ at: 0, held: false, before: null });
  const hits = useMemo(() => omniHits(o.typed, web, arts, recent, now), [o.typed, web, arts, recent, now]);
  const rows = useMemo(() => omniRows(o.typed, hits, o.makes ?? []), [o.typed, hits, o.makes]);
  const field = clampAt({ ...st, typed: o.typed }, rows.length);
  const shown = shownOf(field, rows, hits);
  const set = (f: OmniField) => {
    if (f.typed !== o.typed) o.setTyped(f.typed);
    setSt({ at: f.at, held: f.held, before: f.before });
  };
  // the inline part selected after his words; after a take or a walk, the caret at the end
  const caretEnd = useRef(false);
  useLayoutEffect(() => {
    const el = o.ref.current;
    if (el === null || document.activeElement !== el) return;
    if (shown.inline !== '') el.setSelectionRange(shown.value.length - shown.inline.length, shown.value.length);
    else if (caretEnd.current) el.setSelectionRange(shown.value.length, shown.value.length);
    caretEnd.current = false;
  }, [o.ref, shown.value, shown.inline]);
  const open = (r: OmniRow, behind: boolean) => {
    const bise = (window as Partial<Window>).bise;
    switch (r.kind) {
      case 'web':
      case 'url':
      case 'search': {
        const url = r.kind === 'web' ? r.page.url : r.url;
        void bise?.openExternal(url, { typed: true, ...(behind ? { background: true } : {}) });
        // the page went to his browser: the New tab's job is done (designer m_64584 (1))
        if (!behind) closeTabId(o.tabId);
        return;
      }
      case 'found': {
        const e = { kind: 'artifact' as const, project: r.item.project, id: r.item.a.id };
        if (!behind) {
          openWhat({ kind: 'thing', e }, o.tabId);
          return;
        }
        // a new tab behind this one: it opens, then this tab comes back to the front
        openWhat({ kind: 'thing', e });
        sayOpened(getAppStore(), e.project, e.id);
        activateTab(o.tabId);
        return;
      }
      case 'new':
        o.make?.(r.make);
    }
  };
  return {
    field,
    hits,
    rows,
    value: shown.value,
    input: (value, ev) => {
      // a letter typed (Chromium's insertText); a delete, a paste, IME's composing: no inline (a change with no inputType: a letter)
      const how = (ev?.inputType ?? 'insertText') === 'insertText' && ev?.isComposing !== true ? 'insert' : 'other';
      set(onInput(value, how));
    },
    key: (e) => {
      if (e.nativeEvent.isComposing) return false;
      const out = onKey(field, { key: e.key, shift: e.shiftKey, mod: e.metaKey || e.ctrlKey }, rows, hits);
      if (out === null) return false;
      e.preventDefault();
      if (e.key === 'Escape') e.stopPropagation();
      caretEnd.current = true;
      set(out.f);
      const r = rows[out.f.at];
      if (out.do === 'remove' && r?.kind === 'web') removeWebPage(r.page.url);
      else if ((out.do === 'open' || out.do === 'open-behind') && r !== undefined) open(r, out.do === 'open-behind');
      return true;
    },
    open,
    point: (i) => {
      if (i !== field.at) setSt({ at: i, held: field.held, before: field.before });
    },
  };
}

/** A row's icon, words and test id. */
function rowLook(r: OmniRow): { readonly icon: ReactNode; readonly title: string; readonly sub: string; readonly key: string } {
  switch (r.kind) {
    case 'web':
      return { icon: <AppIcon name="browser" size={16} />, title: shownUrl(r.page.url), sub: panel.omni_web({ host: hostOf(r.page.url) }), key: `web.${r.page.url}` };
    case 'found':
      return { icon: <KindIcon kind={iconKindOf(r.item.a)} size={16} />, title: r.item.a.title, sub: panel.launch_kind_by({ kind: r.item.a.kind, who: r.item.a.agent }), key: `found.${r.item.project}/${r.item.a.id}` };
    case 'url':
      return { icon: <AppIcon name="browser" size={16} />, title: r.url, sub: panel.addr_out, key: 'url' };
    case 'search':
      return { icon: <AppIcon name="search" size={16} />, title: panel.omni_search({ q: r.q }), sub: '', key: 'search.row' };
    case 'new':
      return { icon: <AppIcon name="file" size={16} />, title: r.make.title, sub: r.make.app, key: `new.${makeKey(r.make)}` };
  }
}

/** The omnibox's rows; `prefix` their test ids' ('app.panel.launcher'), `optId` each option's id. */
export function OmniList({ omni, prefix, optId }: { omni: Omni; prefix: string; optId: (i: number) => string }) {
  return (
    <div className="pn-found">
      {omni.rows.map((r, i) => {
        const l = rowLook(r);
        const pick = (behind: boolean) => {
          omni.open(r, behind);
        };
        const row = (
          <PickRow
            key={l.key}
            id={optId(i)}
            on={i === omni.field.at}
            icon={l.icon}
            title={l.title}
            sub={l.sub}
            onPick={pick}
            onHover={() => {
              omni.point(i);
            }}
            testId={`${prefix}.${l.key}`}
          />
        );
        if (r.kind !== 'web') return row;
        // a page's × on hover: out of his history (a sibling: never a button in a button)
        return (
          <div key={l.key} className="pn-omni-web">
            {row}
            <span className="pn-omni-x">
              <IconButton
                label={panel.omni_remove}
                size="row"
                onClick={() => {
                  removeWebPage(r.page.url);
                }}
                testId={`${prefix}.${l.key}.remove`}
              >
                <AppIcon name="close" size={14} />
              </IconButton>
            </span>
          </div>
        );
      })}
    </div>
  );
}
