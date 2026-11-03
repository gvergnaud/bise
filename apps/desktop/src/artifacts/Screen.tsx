// The artifacts screen, in folders (bise fait mac: page native-mac §6, mockup 7, decision d2;
// artifacts-folders brief part 4, page artifacts-gallery v7; route artifacts[/<p>]): one library
// for every project. The window's bar holds the place's name (its crumb) alone. Under the bar the
// banner (ds PageBanner, the bims filing desk as its vignette) scrolls away with the page; then the
// toolbar over the list (Toolbar.tsx, sticky: the kinds the place holds, the project scope or
// 'in:<project>' typed; cards or a list, the search, + add), the place he is in, and the archived
// ones' faint row under it: all artifacts (the Recent row of the 20 newest, Recent.tsx; the themes as big tiles, then every artifact by day), recent, unfiled, a theme
// (its task folders as small tiles, then its cards), a task's folder (its cards by day). The
// library (Column.tsx) draws in the window's sidebar (ArtLibrary, library.tsx: this page publishes
// its props there); with no sidebar library, in the page's 200 px column (under 900 px folded into
// a crumb, Fold.tsx). / searches every folder (Search.tsx). A card moves by a drag onto a folder or a tile (drag.ts), by m (MovePicker.tsx),
// by its right-click menu (Menus.tsx); ⌘/shift-click picks several, the selection bar comes up.
// Folders: + new folder in the column, F2 or a double-click renames, the folder menu merges or
// deletes (a dialog). Every move and folder edit has its undo (⌘Z, and the toast's). The grid, the
// cards, the stacks and the list view are art-grid's (Grid.tsx, Sheet.tsx). Owner: art-folders.
import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type MouseEvent } from 'react';
import { Button, Empty, GroupLabel, MoreButton, PageBanner, Press, sentenceCase, Sheet, tipAttrs, Toast, type EntityMenu } from '@/ds';
import { hero } from '@/proto/words_hero.gen';
import { library } from '@/proto/words_library.gen';
import { useFolders, useProjects, useStore } from '@/store';
import { DEPTH, useRouter } from '@/app/router';
import { libraryExit } from '@/app/escape';
import { usePageSearchKeys } from '@/app/searchkeys';
import { publishColumn, retractColumn, useLibraryShown, type ColumnBits } from './library';
import { useKeptScroll } from '@/app/keep';
import { usePins } from '@/app/pins';
import { useAllArtifacts } from './hooks';
import { rowKey, useLook } from './List';
import { usePutInMessage } from './Open';
import { leaves, useGoArtifact } from './leave';
import { openTab } from '@/app/panel/tabs';
import { AddButton, AddDrop, useAddPlace } from './Drops';
import { ArtifactGrid } from './Grid';
import { StackSheet } from './Sheet';
import { useGridKeys, type View } from './gridKeys';
import { itemsOf, ownerOf, topOf, whenOf, type Card, type Place, type Stack } from './gallery';
import { Thumb } from './thumbs';
import { keyOf, revealPathOf, splitArchived, type Item } from './model';
import {
  AT_ALL, atOfPath, atPath, chipOf, chipsOf, countsOf, coverOf, crumbOf, folderLine, inFolder, itemsAt, libraryFacts, libraryOf,
  nameOf, parseQuery, pathOf, projectOfTyped, rememberMove, searchItems, titlePath, type At, type ChipKind, type Library,
} from './folders';
import { FolderColumn, type Editing } from './Column';
import { chipWord } from './folders';
import { FoldCrumb, useNarrow } from './Fold';
import { sayCrumbSearch } from './crumb';
import { MovePicker } from './MovePicker';
import { atOf, CardMenu, FolderMenu, MergePicker, type CardAct, type FolderAct, type MenuAt } from './Menus';
import { ProjectScope, scopeRows } from './Scope';
import { SearchResults } from './Search';
import { Toolbar } from './Toolbar';
import { BigTile, FolderListRow, SmallTile } from './Tiles';
import { cardDrag, FLASH_MS, TOAST_MS, useBrief, useDrop } from './drag';
import { folderCmd, moveCmd, type FolderOp } from './ops';
import { useBannerGutter } from './hero/vignettes';
import { RecentRow } from './Recent';
import { LiveVignette } from './hero/Live';
import './artifacts.css';
import './folders.css';

const RECENT_KEY = 'bise.artifacts.recentFolders';
const readRecent = (): string[] => {
  try {
    const v: unknown = JSON.parse(window.localStorage.getItem(RECENT_KEY) ?? '[]');
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string') : [];
  } catch {
    return [];
  }
};

/** What floats over the page: one at a time. */
type Over =
  | { readonly k: 'move'; readonly keys: readonly string[]; readonly at: MenuAt }
  | { readonly k: 'card'; readonly key: string; readonly at: MenuAt }
  | { readonly k: 'folder'; readonly id: string; readonly at: MenuAt }
  | { readonly k: 'merge'; readonly id: string; readonly at: MenuAt }
  | { readonly k: 'delete'; readonly id: string }
  | null;

/** What the page keeps across its places (each place is a route, the screen mounts anew): the
 *  view, the themes he opened, the ones shown past 'N more'. */
const mem: { view: View; open: ReadonlySet<string>; full: ReadonlySet<string> } = { view: 'grid', open: new Set(), full: new Set() };
const keepView = (v: View) => { mem.view = v; };
const keepOpen = (o: ReadonlySet<string>) => { mem.open = o; };
const keepFull = (f: ReadonlySet<string>) => { mem.full = f; };

export function ArtifactsScreen({ project = '', at: atRoute = '' }: { project?: string; at?: string }) {
  const store = useStore();
  const router = useRouter();
  const all = useAllArtifacts();
  // what he pinned: its own row above Recent, in pin order (app/pins.ts)
  const artPins = usePins('artifact');
  const pinnedOf = (items: readonly Item[]): Item[] =>
    [...artPins].flatMap(([p, ids]) => ids.flatMap((id) => items.filter((it) => it.project === p && it.a.id === id)));
  const folders = useFolders();
  const projects = useProjects();
  useKeptScroll(`artifacts:${project}:${atRoute}`, () => document.querySelector<HTMLElement>('[data-testid="artifacts.scroll"]'));
  const [now] = useState(() => Date.now());
  // the scope and the place are the route's (the title bar's path follows the place)
  const scope = project;
  const at = atOfPath(atRoute);
  const [q, setQ] = useState('');
  const [chip, setChip] = useState<ChipKind | null>(null);
  const [view, setViewRaw] = useState<View>(mem.view);
  const [picked, setPicked] = useState<ReadonlySet<string>>(new Set());
  const [anchorKey, setAnchorKey] = useState<string | null>(null);
  const [open, setOpenRaw] = useState<ReadonlySet<string>>(mem.open);
  const [full, setFullRaw] = useState<ReadonlySet<string>>(mem.full);
  const setView = (v: View) => { keepView(v); setViewRaw(v); };
  const setOpen = (o: ReadonlySet<string>) => { keepOpen(o); setOpenRaw(o); };
  const setFull = (f: ReadonlySet<string>) => { keepFull(f); setFullRaw(f); };
  const [editing, setEditing] = useState<Editing>(null);
  const [over, setOver] = useState<Over>(null);
  const [stack, setStack] = useState<Stack | null>(null);
  // the archived ones are a place of the route ('archived'): 'a' and the toggle switch it in place
  // (no history step, as the toggle was; a new route remounts the page, as a click on a library
  // row), a gone artifact's link opens it (gone.ts)
  const [archivedOn, setArchivedRaw] = useState(at.at === 'archived');
  const setArchivedOn = (on: boolean) => {
    setArchivedRaw(on);
    const path = atPath(on ? { at: 'archived' } : AT_ALL);
    router.replace(path === '' ? { screen: 'artifacts', project: scope } : { screen: 'artifacts', project: scope, at: path });
  };
  const [recent, setRecent] = useState<string[]>(readRecent);
  const [flash, setFlash] = useBrief<string>(FLASH_MS);
  const [toast, setToast] = useBrief<string>(TOAST_MS);
  // a move to a folder made from the picker (⌘⏎): sent once the folder is in the list
  const pendingMove = useRef<{ name: string; parent: string | null; keys: readonly string[] } | null>(null);
  const narrow = useNarrow();
  const searchRef = useRef<HTMLInputElement>(null);
  // he moved the cursor (a key) or picked a card: its ring shows from then on (cards.css)
  const [keyed, setKeyed] = useState(false);
  // the window's sidebar draws the library (ArtLibrary): the page draws no column of its own
  const sideLib = useLibraryShown();
  const gutter = useBannerGutter();

  const go = (p: string, a: At) => {
    const path = atPath(a);
    router.go(path === '' ? { screen: 'artifacts', project: p } : { screen: 'artifacts', project: p, at: path });
  };
  const setAt = (a: At) => {
    if (a.at === 'folder') {
      const theme = folders.find((f) => f.id === a.id)?.parent ?? a.id;
      if (!open.has(theme)) setOpen(new Set([...open, theme]));
    }
    go(scope, a);
  };
  const setScope = (p: string) => { go(p, at); };


  const { live, archived } = splitArchived(all);
  const base = archivedOn ? archived : live;
  const lib = libraryOf(folders, all);
  const parsed = parseQuery(q);
  const typedProject = parsed.project === null ? null : projectOfTyped(projects.map((p) => ({ project: p.project, name: p.name })), parsed.project);
  const eff = typedProject ?? scope;
  const projectName = eff === '' ? '' : (projects.find((p) => p.project === eff)?.name || eff);
  const scoped = eff === '' ? base : base.filter((it) => it.project === eff);
  const counts = countsOf(lib, base, scoped);
  const searching = parsed.words.length > 0;
  // the title bar's path says the search while there is one ('artifacts › search · dark')
  const searchWords = parsed.words.join(' ');
  useEffect(() => { sayCrumbSearch(searchWords); }, [searchWords]);
  useEffect(() => () => { sayCrumbSearch(''); }, []);
  // a folder gone (deleted, merged): back to all artifacts
  const atNow: At = at.at === 'folder' && !lib.byId.has(at.id) ? { at: 'all' } : at;
  const found = searching ? searchItems(lib, scoped, parsed.words) : [];
  const here = searching ? found : itemsAt(lib, scoped, atNow);
  const chips = chipsOf(here);
  const shown = chip === null ? here : here.filter((it) => chipOf(it) === chip);
  const byKey = new Map(base.map((it) => [keyOf(it), it]));
  const place = placeOf(lib, atNow, searching, shown);
  const curId = atNow.at === 'folder' ? atNow.id : null;
  const isTheme = curId !== null && (lib.byId.get(curId)?.parent ?? null) === null;

  const goArtifact = useGoArtifact();
  // one opens in a tab of the panel on the right (plan native-mac-plan v4 §1: the page stays); a
  // browser-only one (a PR, a link) in his browser, as before
  const openIt = (it: Item) => {
    if (leaves(it.a)) goArtifact(it.project, it.a.id, it.a);
    else openTab({ kind: 'artifact', project: it.project, id: it.a.id });
  };
  const look = useLook(openIt);
  const put = usePutInMessage();
  const putIt = (it: Item) => { put(it.project, it.a); };
  const place_ = useAddPlace(eff);

  const send = (cmd: object) => { store.command(cmd as never); };
  const edit = (op: FolderOp) => { send(folderCmd(op)); };
  // every one the kinds' count says, all artifacts too (his m_44961: 'All' showed the newest 40 alone)
  const { sel, setSel, ref, onKeyDown, cards, cols, cardOf } = useGridKeys({
    items: shown,
    view,
    days: atNow.at !== 'recent',
    keepAs: `artifacts.sel:${project}`,
    onOpen: (k) => { openCard(k); },
    onKey: (e, k) => rowKey(e, k === null ? undefined : itemOfKey(k), { onOpen: openIt, onLook: look, onPut: putIt }) || screenKey(e, k),
  });
  function itemOfKey(k: string): Item | undefined {
    const c = cardOf(k);
    return c === undefined ? byKey.get(k) : topOf(c);
  }
  function openCard(k: string) {
    const c = cardOf(k);
    if (c === undefined) return;
    if (c.kind === 'stack') setStack(c);
    else openIt(c.it);
  }
  const itemsOfKeys = (keys: readonly string[]): Item[] => keys.flatMap((k) => {
    const c = cardOf(k);
    if (c !== undefined) return [...itemsOf(c)];
    const it = byKey.get(k);
    return it === undefined ? [] : [it];
  });
  function moveTo(keys: readonly string[], folder: string) {
    const items = itemsOfKeys(keys);
    if (items.length === 0) return;
    const to = folder === 'unfiled' ? null : folder;
    send(moveCmd(items, to, lib.folderOf));
    if (to !== null) {
      const next = rememberMove(recent, to);
      setRecent(next);
      try { window.localStorage.setItem(RECENT_KEY, JSON.stringify(next)); } catch { /* private window */ }
    }
    setFlash(folder);
    const what = items.length === 1 ? (items[0]?.a.title ?? '') : library.n_artifacts({ n: items.length });
    setToast(library.moved({ what, to: to === null ? library.unfiled : nameOf(lib, to) }));
    setPicked(new Set());
    setOver(null);
  }
  // a folder made from the m picker: move there once it is in the list
  useEffect(() => {
    const pm = pendingMove.current;
    if (pm === null) return;
    const made = folders.find((f) => f.name === pm.name && (f.parent ?? null) === pm.parent);
    if (made === undefined) return;
    pendingMove.current = null;
    moveTo(pm.keys, made.id);
  });

  const undo = () => {
    edit('undo');
    setToast(null);
  };

  const keysToMove = (sel: string | null): string[] => (picked.size > 0 ? [...picked] : sel === null ? [] : [sel]);
  // the screen's own keys, beside the grid's and the card's
  function screenKey(e: KeyboardEvent, sel: string | null): boolean {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'z') {
      undo();
      return true;
    }
    if (e.metaKey || e.ctrlKey || e.altKey) return false;
    if (e.key === 'm') {
      const keys = keysToMove(sel);
      if (keys.length > 0) setOver({ k: 'move', keys, at: boxOf(keys[0] ?? '') });
      return true;
    }
    if (e.key === 'a') {
      setArchivedOn(!archivedOn);
      setPicked(new Set());
      return true;
    }
    if (e.key !== 'Escape') return false;
    if (picked.size > 0) setPicked(new Set());
    else if (stack !== null) setStack(null);
    else if (q !== '') setQ('');
    else if (archivedOn) setArchivedOn(false);
    else if (atNow.at !== 'all') {
      // esc goes up, never back down (designer m_44802): the parent replaces this step (‹ still
      // reaches the folder); at the root, out to where he was before the library (app/escape.ts)
      const p = atNow.at === 'folder' ? (lib.byId.get(atNow.id)?.parent ?? null) : null;
      const path = atPath(p === null ? { at: 'all' } : { at: 'folder', id: p });
      router.replace(path === '' ? { screen: 'artifacts', project: scope } : { screen: 'artifacts', project: scope, at: path });
    } else {
      const n = libraryExit(router.places('back', DEPTH));
      if (n === null) return false;
      router.jump('back', n);
    }
    return true;
  }
  // the card's box (the picker opens from it), else the page's top
  const boxOf = (key: string): MenuAt => {
    const el = document.querySelector(`[data-row="${CSS.escape(key)}"]`) ?? ref.current;
    return el === null ? { x: 0, y: 0 } : atOf(el);
  };
  const onPick = (key: string, how: { meta: boolean; shift: boolean }) => {
    setKeyed(true);
    if (how.shift && anchorKey !== null) {
      const order = cards.map((c) => c.key);
      const [a, b] = [order.indexOf(anchorKey), order.indexOf(key)].sort((x, y) => x - y);
      if (a !== undefined && b !== undefined && a >= 0) setPicked(new Set(order.slice(a, b + 1)));
      setSel(key);
      return;
    }
    if (how.meta) {
      const next = new Set(picked);
      if (picked.size === 0 && sel !== null && sel !== key) next.add(sel);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      setPicked(next);
      setAnchorKey(key);
      setSel(key);
      return;
    }
    setPicked(new Set());
    setAnchorKey(key);
    setSel(key);
  };
  const drop = useDrop({
    isClosedTheme: (id) => (lib.tasks.get(id)?.length ?? 0) > 0 && !open.has(id),
    onOpen: (id) => { setOpen(new Set([...open, id])); },
    onDrop: (keys, id) => { moveTo(keys, id); },
  });

  // '/' (outside a text field) and ⌘F focus the search, wherever the focus is on the page
  usePageSearchKeys(searchRef);
  // the search has the keys: ⏎ opens the first result, ↓ the first card, ↑ gives them back; esc
  // clears its words, a second esc leaves it (designer's spec D)
  const searchKey = (e: KeyboardEvent<HTMLInputElement>) => {
    const back = () => {
      e.preventDefault();
      ref.current?.focus({ preventScroll: true });
    };
    if (e.key === 'Escape') {
      if (q !== '') {
        e.preventDefault();
        setQ('');
      } else back();
    } else if (e.key === 'Enter') {
      const first = shown[0];
      if (searching && first !== undefined) openIt(first);
      back();
    } else if (e.key === 'ArrowDown') {
      back();
      setKeyed(true);
      setSel(cards[0]?.key ?? null);
    } else if (e.key === 'ArrowUp') back();
  };
  const doFolder = (id: string, act: FolderAct, at_: MenuAt) => {
    if (act === 'rename') setEditing({ kind: 'rename', id });
    else if (act === 'inside') {
      setOpen(new Set([...open, id]));
      setEditing({ kind: 'new', parent: id });
    } else if (act === 'merge') setOver({ k: 'merge', id, at: at_ });
    else setOver({ k: 'delete', id });
  };
  const doCard = (key: string, act: CardAct) => {
    const it = itemOfKey(key);
    if (it === undefined) return;
    const keys = picked.has(key) ? [...picked] : [key];
    switch (act) {
      case 'open': openCard(key); return;
      case 'look': look(it); return;
      case 'put': for (const x of itemsOfKeys(keys)) putIt(x); return;
      case 'move': setOver({ k: 'move', keys, at: boxOf(key) }); return;
      default:
        // versions, copy, finder, archive: the card's own keys (List.tsx rowKey)
        setSel(key);
        rowKey(new KeyboardEvent('keydown', { key: { versions: 'v', copy: 'c', finder: 'r', archive: 'Backspace' }[act] }) as never, it, { onOpen: openIt, onLook: look, onPut: putIt });
    }
  };
  const onEdited = (name: string | null) => {
    const e = editing;
    setEditing(null);
    if (e === null || name === null) return;
    if (e.kind === 'rename') edit({ rename: { id: e.id, name } });
    else edit({ make: e.parent === null ? { name } : { name, parent: e.parent } });
  };


  // the place's theme is always open in the column (a folder reached by a link, a tile, the fold)
  const curTheme = curId === null ? null : (lib.byId.get(curId)?.parent ?? curId);
  const shownOpen = curTheme === null || open.has(curTheme) ? open : new Set([...open, curTheme]);
  // the library's props, one copy: the page's column draws them, or the sidebar's (library.tsx)
  const colBits: ColumnBits = {
    lib, counts, at: atNow, open: shownOpen, full, editing, flash, drop,
    onGo: (a) => { setQ(''); setAt(a); }, onToggle: (t) => { setOpen(toggled(open, t)); }, onMore: (t) => { setFull(new Set([...full, t])); },
    onNew: () => { setEditing({ kind: 'new', parent: null }); }, onRename: (id) => { setEditing({ kind: 'rename', id }); }, onEdited,
    onMenu: (id, e: MouseEvent) => { setOver({ k: 'folder', id, at: { x: e.clientX, y: e.clientY } }); },
  };
  // what this page published last: its leave clears the library only if no newer page published since
  const published = useRef<ColumnBits | null>(null);
  useLayoutEffect(() => { published.current = colBits; publishColumn(colBits); });
  useEffect(() => () => { retractColumn(published.current); }, []);
  const column = <FolderColumn {...colBits} testId="artifacts.col" />;
  const grid = (items: readonly Card[], placeHere: Place, days: boolean, testId: string) => (
    <ArtifactGrid cards={items} cols={cols} view={view} now={now} place={placeHere} days={days} cursor={sel} picked={picked}
      onPick={onPick} onOpen={(c) => { openCard(c.key); }}
      onMenu={(key, p) => { setOver({ k: 'card', key, at: p }); }}
      dragOf={(key) => cardDrag(key, picked)}
      scroller={scrollEl} testId={testId} />
  );
  const pickScope = (p: string) => { setScope(p); if (typedProject !== null) setQ(parsed.words.join(' ')); };
  const crumb = crumbOf(lib, atNow);
  // under 900 px the column folds into the crumb: the place's title opens it (page v7 'à 800 px')
  const fold = <FoldCrumb label={crumb.length === 0 ? sentenceCase(library.all_artifacts) : crumb.map((c) => c.name).join(' / ')} testId="artifacts.fold">{column}</FoldCrumb>;
  const current = atNow.at === 'folder' ? lib.byId.get(atNow.id) : undefined;
  // the toolbar above the list (his m_44961, designer's spec D): the kinds the place holds (all,
  // then each kind with its count), the project scope; the cards/list switch, the search ('/'), +
  const kindsTotal = searching ? found.length : here.length;
  const toolbar = stack === null && (
    <Toolbar testId="artifacts.toolbar" lead={narrow ? fold : null}
      kinds={searching || chips.length > 1 ? [{ id: 'all', word: library.all_kinds, n: kindsTotal }, ...chips.map((c) => ({ id: c.kind, word: chipWord(c.kind), n: c.n }))] : null}
      kind={chip ?? 'all'} onKind={(id) => { setChip(id === 'all' ? null : (id as ChipKind)); }}
      scope={<ProjectScope all={base} project={eff} projectName={projectName} onPick={pickScope} testId="artifacts.projects" />}
      projects={[{ project: '', name: sentenceCase(library.all_projects), n: base.length }, ...scopeRows(base)]} project={eff} onProject={pickScope}
      view={view} onView={setView}
      q={q} onQ={(e) => { setQ(e.currentTarget.value); }} onQKey={searchKey}
      onClear={() => { setQ(''); searchRef.current?.focus(); }} searchRef={searchRef}
      add={<AddButton place={place_} tip={library.add_tip} testId="artifacts.add" />} />
  );
  // the archived ones: a faint row under the list, as the window's panel and the TUI (designer m_45148)
  const archivedRow = stack === null && archived.length > 0 && (
    <div className="art-archived-row">
      <Press look="art-archived" aria-pressed={archivedOn} onClick={() => { setArchivedOn(!archivedOn); setPicked(new Set()); }} data-testid="artifacts.archived"
        {...tipAttrs(library.archived_tip)}>
        {library.archived_n({ n: archived.length })}
      </Press>
    </div>
  );
  // the place's folder: its '…' (the folder's own menu: rename, a folder inside, merge, delete)
  const folderMenu: EntityMenu | null = current === undefined ? null : {
    onContextMenu: (e) => { e.preventDefault(); setOver({ k: 'folder', id: current.id, at: { x: e.clientX, y: e.clientY } }); },
    open: () => {
      const el = document.querySelector('[data-testid="artifacts.head.more"]');
      setOver({ k: 'folder', id: current.id, at: el === null ? { x: 0, y: 0 } : atOf(el) });
    },
  };

  const body = (() => {
    if (all.length === 0) return <FirstRun place={place_} />;
    if (archivedOn && base.length === 0) return <Empty title={library.none_archived} testId="artifacts.none" />;
    if (stack !== null) {
      return <StackSheet stack={stack} now={now} onOpen={openIt} onLook={look} onPut={putIt} onBack={() => { setStack(null); }} testId="artifacts.sheet" />;
    }
    if (searching) {
      if (found.length === 0) {
        return (
          <Empty title={library.nothing_matches({ q: searchWords })} testId="artifacts.nothing"
            action={<Button kind="ghost" onClick={() => { setQ(''); searchRef.current?.focus(); }} testId="artifacts.nothing.clear">{library.clear_search}</Button>} />
        );
      }
      // a filter on the same list (designer m_45293): every match, under the same day heads as All
      return (
        <SearchResults q={searchWords} total={found.length} testId="artifacts.results">
          {grid(cards, place, true, 'artifacts.grid')}
        </SearchResults>
      );
    }
    if (atNow.at === 'all') {
      const themes = lib.themes.filter((t) => eff === '' || (counts.get(t.id)?.n ?? 0) > 0);
      return (
        <>
          {/* the 20 newest, whatever their folder (page art-recent): the scope and the kind chip apply */}
          {/* what he pinned (app/pins.ts, his m_49100), above Recent: the scope and the kind chip apply */}
          <RecentRow kind="pinned" items={pinnedOf(shown)} lib={lib} now={now} list={view === 'list'} onOpen={openIt} testId="artifacts.pinned" />
          <RecentRow items={shown} lib={lib} now={now} list={view === 'list'} onOpen={openIt}
            onAll={() => { setAt({ at: 'recent' }); }} testId="artifacts.recent" />
          {themes.length > 0 && (
            <>
              <GroupLabel tip={library.recent_first} testId="artifacts.sect.folders">{library.folders}</GroupLabel>
              {view === 'list' ? (
                <div className="art-frows" data-testid="artifacts.tiles">
                  {themes.map((t) => (
                    <FolderListRow key={t.id} folder={t} count={counts.get(t.id)} when={whenOf(lib.latest.get(t.id) ?? now, now)} drop={drop} flash={flash === t.id}
                      onOpen={() => { setAt({ at: 'folder', id: t.id }); }} onMenu={(e) => { setOver({ k: 'folder', id: t.id, at: { x: e.clientX, y: e.clientY } }); }}
                      testId={`artifacts.tile.${t.id}`} />
                  ))}
                </div>
              ) : (
                <>
                  <div className="art-tiles big" data-testid="artifacts.tiles">
                    {themes.filter((t) => (counts.get(t.id)?.total ?? 0) > 0).map((t) => (
                      <BigTile key={t.id} folder={t} count={counts.get(t.id)} when={whenOf(lib.latest.get(t.id) ?? now, now)}
                        cover={coverOf(lib, scoped, t.id)} thumb={(it) => <Thumb item={it} visible />} drop={drop} flash={flash === t.id}
                        onOpen={() => { setAt({ at: 'folder', id: t.id }); }} onMenu={(e) => { setOver({ k: 'folder', id: t.id, at: { x: e.clientX, y: e.clientY } }); }}
                        testId={`artifacts.tile.${t.id}`} />
                    ))}
                  </div>
                  {/* an empty folder is a row, never a tile with nothing in it (app-polish #14) */}
                  <div className="art-frows" data-testid="artifacts.tiles.empty">
                    {themes.filter((t) => (counts.get(t.id)?.total ?? 0) === 0).map((t) => (
                      <FolderListRow key={t.id} folder={t} count={counts.get(t.id)} when={whenOf(lib.latest.get(t.id) ?? now, now)} drop={drop} flash={flash === t.id}
                        onOpen={() => { setAt({ at: 'folder', id: t.id }); }} onMenu={(e) => { setOver({ k: 'folder', id: t.id, at: { x: e.clientX, y: e.clientY } }); }}
                        testId={`artifacts.tile.${t.id}`} />
                    ))}
                  </div>
                </>
              )}
            </>
          )}
          {/* every one, newest first, under its day's head (designer m_45148: no 'Recent' label) */}
          {grid(cards, place, true, 'artifacts.grid')}
        </>
      );
    }
    if (atNow.at === 'unfiled' && here.length === 0) {
      return <Empty title={library.no_unfiled_title} testId="artifacts.none">{library.no_unfiled_say}</Empty>;
    }
    if (atNow.at === 'folder' && here.length === 0) {
      if (eff !== '' && (counts.get(atNow.id)?.total ?? 0) > 0) {
        return <Empty title={library.none_from({ project: projectName })} testId="artifacts.none" />;
      }
      // the empty folder (app-polish #14): the ds Empty, on the column's edge
      return <Empty title={library.empty_folder_title} testId="artifacts.none">{library.empty_folder_say({ name: nameOf(lib, atNow.id) })}</Empty>;
    }
    const tasks = isTheme ? (lib.tasks.get(curId) ?? []) : [];
    return (
      <>
        {tasks.length > 0 && (
          // in the ☰ list the task folders are rows on top; in the grid, small tiles
          <div className={view === 'list' ? 'art-frows' : 'art-tiles small'} data-testid="artifacts.tasks">
            {tasks.filter((f) => eff === '' || (counts.get(f.id)?.n ?? 0) > 0).map((f) => {
              const tp = {
                folder: f, count: counts.get(f.id), drop, flash: flash === f.id,
                onOpen: () => { setAt({ at: 'folder', id: f.id }); }, onMenu: (e: MouseEvent) => { setOver({ k: 'folder', id: f.id, at: { x: e.clientX, y: e.clientY } }); },
                testId: `artifacts.task.${f.id}`,
              };
              return view === 'list'
                ? <FolderListRow key={f.id} {...tp} when={whenOf(lib.latest.get(f.id) ?? now, now)} />
                : <SmallTile key={f.id} {...tp} when="" />;
            })}
          </div>
        )}
        {grid(cards, place, atNow.at !== 'recent', 'artifacts.grid')}
      </>
    );
  })();

  const head = searching || atNow.at === 'all' || stack !== null ? null : (
    // the place's name is the window's bar's (its crumb: 'artifacts › launch'), never a second time
    // here (designer's rule on #5); under 900 the fold's crumb opens the library
    <header className="art-head" data-testid="artifacts.head" onContextMenu={folderMenu?.onContextMenu}>
      <p className="art-head-s">
        {folderMenu !== null && <MoreButton menu={folderMenu} testId="artifacts.head.more" />}
        {atNow.at === 'folder' ? folderLine(lib, atNow.id, here) : atNow.at === 'unfiled' ? library.unfiled_say : ''}
        {atNow.at === 'unfiled' && here.length > 0 && (
          <> <Press look="art-link" onClick={() => { router.go({ screen: 'bise' }); }} data-testid="artifacts.sort">{library.sort_with_main}</Press></>
        )}
      </p>
    </header>
  );
  // a stack open: its path alone, 'desktop app / agent view / agent view step 2' (the sheet has its
  // own line and chips: no scope, search or kind chips over it); under 900 the path opens the column
  const stackPath = stack === null ? '' : [...crumb.map((c) => c.name), stack.title].join(' / ');
  const stackHead = stack === null ? null : (
    <header className="art-head" data-testid="artifacts.head">
      <div className="art-head-t">
        {narrow ? <FoldCrumb label={stackPath} testId="artifacts.fold">{column}</FoldCrumb> : <span className="t">{stackPath}</span>}
      </div>
    </header>
  );

  return (
    <section ref={ref} tabIndex={-1} className="art-screen" data-testid="artifacts.screen" data-archived={archivedOn || undefined} data-narrow={narrow || undefined}
      data-title-path={titlePath(lib, atNow, searchWords)} data-keyed={keyed || undefined}
      onKeyDown={(e) => {
        // the cursor's ring shows once he moves it (a key) or picks a card, never at load (ui-review)
        if (!keyed && (e.key.startsWith('Arrow') || e.key === 'j' || e.key === 'k' || e.key === 'Home' || e.key === 'End')) setKeyed(true);
        onKeyDown(e);
      }}>
      <div className="art-scroll" data-testid="artifacts.scroll">
        {/* the lead (designer's rule on #5): no card; the bims still stays beside it as a vignette */}
        {/* the banner (designer m_42762): a full-width page, its title 32 in, the filing desk right */}
        <PageBanner title={hero.artifacts_title} sentence={library.say} facts={libraryFacts(lib, scoped, projectName)}
          visual={<LiveVignette page="artifacts" />} icon="artifacts" edge="page" gutter={gutter} testId="artifacts.lead" />
        <div className={`art-lib${narrow || sideLib ? ' bare' : ''}`}>
          {!narrow && !sideLib && column}
          <div className="art-main">
            {toolbar}
            {stackHead ?? head}
            <AddDrop place={place_} testId="artifacts.drop">{body}</AddDrop>
            {archivedRow}
          </div>
        </div>
      </div>
      {picked.size > 0 && (
        <div className="art-selbar" data-testid="artifacts.selbar">
          <span className="n">{library.picked({ n: picked.size })}</span>
          <Press look="art-selbar-a" onClick={(e) => { setOver({ k: 'move', keys: [...picked], at: atOf(e.currentTarget) }); }} data-testid="artifacts.selbar.move">{library.menu_move}<span className="k">m</span></Press>
          <Press look="art-selbar-a" onClick={() => { for (const x of itemsOfKeys([...picked])) putIt(x); }} data-testid="artifacts.selbar.put">{library.put_in_message}<span className="k">@</span></Press>
          <span className="sep" />
          <Press look="art-selbar-a" onClick={() => { setPicked(new Set()); }} data-testid="artifacts.selbar.esc">esc</Press>
        </div>
      )}
      {toast !== null && picked.size === 0 && (
        <div className="art-toast">
          <Toast action={{ text: `${library.undo} ⌘Z`, onClick: undo, testId: 'artifacts.toast.undo' }} testId="artifacts.toast">{toast}</Toast>
        </div>
      )}
      {over?.k === 'move' && (
        <MovePicker lib={lib} at={over.at} recent={recent} testId="artifacts.move"
          title={over.keys.length === 1 ? library.move_to({ name: itemOfKey(over.keys[0] ?? '')?.a.title ?? '' }) : library.move_n_to({ n: itemsOfKeys(over.keys).length })}
          from={commonFolder(lib, itemsOfKeys(over.keys))}
          onMove={(f) => { moveTo(over.keys, f); }}
          onMake={(name, parent) => {
            pendingMove.current = { name, parent, keys: over.keys };
            edit({ make: parent === null ? { name } : { name, parent } });
            setOver(null);
          }}
          onClose={() => { setOver(null); ref.current?.focus({ preventScroll: true }); }} />
      )}
      {over?.k === 'card' && (
        <CardMenu at={over.at} several={picked.has(over.key) ? picked.size : 1} finder={(() => { const it = itemOfKey(over.key); return it !== undefined && revealPathOf(it.a) !== null; })()}
          onAct={(a) => { doCard(over.key, a); }} onClose={() => { setOver((o) => (o?.k === 'card' ? null : o)); }} testId="artifacts.menu" />
      )}
      {over?.k === 'folder' && (
        <FolderMenu at={over.at} isTheme={(lib.byId.get(over.id)?.parent ?? null) === null}
          onAct={(a) => { doFolder(over.id, a, over.at); }} onClose={() => { setOver((o) => (o?.k === 'folder' ? null : o)); }} testId="artifacts.fmenu" />
      )}
      {over?.k === 'merge' && (
        <MergePicker at={over.at} lib={lib} id={over.id} n={counts.get(over.id)?.total ?? 0}
          onMerge={(into) => { edit({ merge: { from: over.id, into } }); setFlash(into); setToast(library.moved({ what: nameOf(lib, over.id), to: nameOf(lib, into) })); }}
          onClose={() => { setOver((o) => (o?.k === 'merge' ? null : o)); }} testId="artifacts.merge" />
      )}
      <Sheet open={over?.k === 'delete'} onClose={() => { setOver(null); }} testId="artifacts.delete"
        title={over?.k === 'delete' ? library.delete_q({ name: nameOf(lib, over.id) }) : ''}
        sub={over?.k === 'delete' ? library.delete_say({ n: counts.get(over.id)?.total ?? 0 }) : ''}
        onPrimary={() => { if (over?.k === 'delete') { edit({ delete: { id: over.id } }); setOver(null); } }}
        foot={
          <>
            <Button kind="ghost" onClick={() => { setOver(null); }} testId="artifacts.delete.cancel">{library.cancel}</Button>
            <Button kind="danger" onClick={() => { if (over?.k === 'delete') { edit({ delete: { id: over.id } }); setOver(null); } }} testId="artifacts.delete.go">{library.delete_folder}</Button>
          </>
        }>
        {null}
      </Sheet>
    </section>
  );
}

/** The page's one scroller (the grid's virtual rows and the toolbar's stick read it). */
const scrollEl = (): HTMLElement | null => document.querySelector<HTMLElement>('[data-testid="artifacts.scroll"]');

const toggled = (s: ReadonlySet<string>, k: string): Set<string> => {
  const n = new Set(s);
  if (n.has(k)) n.delete(k);
  else n.add(k);
  return n;
};

/** The folder all of them are in, else null (the picker leaves it out). Pure. */
function commonFolder(lib: Library, items: readonly Item[]): string | null {
  const ids = new Set(items.map((it) => lib.folderOf(it)));
  return ids.size === 1 ? ([...ids][0] ?? null) : null;
}

/** What the cards' line leaves out here (density rule 1, art-grid's lineOf). Pure. */
function placeOf(lib: Library, at: At, searching: boolean, items: readonly Item[]): Place {
  if (searching) return { kind: 'search' };
  switch (at.at) {
    case 'all':
    case 'recent':
    case 'archived':
      return { kind: 'recent', folderOf: (it) => { const f = lib.folderOf(it); return f === null ? '' : pathOf(lib, f); } };
    case 'unfiled':
      return { kind: 'unfiled' };
    case 'folder': {
      const isTheme = (lib.byId.get(at.id)?.parent ?? null) === null && (lib.tasks.get(at.id)?.length ?? 0) > 0;
      if (isTheme) return { kind: 'theme', folderOf: (it) => { const f = lib.folderOf(it); return f === null || f === at.id ? '' : nameOf(lib, f); } };
      const owner = ownerOf(items.filter((it) => inFolder(lib, it, at.id)));
      return owner === undefined ? { kind: 'folder' } : { kind: 'folder', owner };
    }
  }
}

/** First run: nothing yet, one sentence, one action. */
function FirstRun({ place }: { place: ReturnType<typeof useAddPlace> }) {
  return (
    <Empty title={library.first_title} action={<AddButton place={place} labeled testId="artifacts.empty.add" />} testId="artifacts.empty">
      {library.first_say}
    </Empty>
  );
}
