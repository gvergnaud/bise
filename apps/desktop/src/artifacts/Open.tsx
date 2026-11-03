// One artifact open (designer's artifact-open v2; amb-win's seam m_19613): ArtifactOpen fills the
// box the frame gives it, the right column at 55% beside its maker's thread (or the whole width
// under 1000 px, and the centre route artifacts/<p>/<id> until the frame wires the column). Its
// head (title, version menu, '↗ browser', ⋯, ✕), then the page from the pages server, 16 px in on
// a 3% backdrop. A link inside the page goes to his browser (the shell's guard, main's
// framelink.ts); one that names another artifact opens it here (links.ts). The composer's
// words come from maker.ts. `bare`: no head, for a panel tab's address bar (bise fait mac,
// nm-panel's Browser.tsx), which picks the version with ArtifactVersions. Owner: amb-kit (the list, the model); task-panel (the open view).
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type ReactNode, type RefObject } from 'react';
import { APP_FRAME_NAME } from './frameNote';
import { useFrameNote } from './useFrameNote';
import { Button, Empty, IconButton, Kbd, PickerPop, PickerTrigger, Press } from '@/ds';
import { frame as frameWords } from '@/proto/words_frame.gen';
import { library } from '@/proto/words_library.gen';
import { panel } from '@/proto/words_panel.gen';
import { useModHeld } from '@/app/held';
import { PRIMARY_KEY, useArtifactFront } from './front';
import { findable, useArtifactFind } from './find';
import { ArtifactFindBar } from './FindBar';
import type { Artifact, ArtifactLive } from '@/proto';
import { useRouter } from '@/app/router';
import { useAgents, usePrefs, useProjects, useStore } from '@/store';
import { turnRuns } from '@/agents/model';
import { downWords, probed, useReach, type DownWords } from './reach';
import { themeOf } from '@/theme';
import { answerOf, ThreadComposer } from '@/thread/ThreadComposer';
import { useTabPlace } from '@/thread/ComposerTab';
import { usageLine } from '@/thread/head';
import { InboxBlock, type Answering } from '@/inbox';
import { useAllArtifacts } from './hooks';
import { kindOf, revealPathOf, saveOf, versions } from './model';
import { useMenuAction } from '@/app/menu';
import { tilde } from '@/app/barplaces';
import type { SaveResult } from '@/shared/api';
import { putInDraft } from './put';
import { keptStorage } from '@/app/keep';
import { askCaret } from '@/composer/caret';
import { useAboutWords } from './maker';
import { goneLinkOf, goneRoute } from './gone';
import { takeVersion, useArtifactLinks } from './links';
import './artifacts.css';

/** What the window's bridge offers here (each one absent in a plain browser or a test: its item hides). */
export interface ArtBridge {
  openExternal?: (u: string) => unknown;
  copy?: (t: string) => unknown;
  quickLook?: (path: string) => Promise<boolean>;
  showInFinder?: (path: string) => Promise<boolean>;
  saveCopy?: (path: string, name: string) => Promise<SaveResult>;
  home?: string;
}
export const bridge = (): ArtBridge | undefined => (globalThis as { bise?: ArtBridge }).bise;

/** '@ put it in a message': the artifact as a chip in its agent's draft, then that thread. */
export function usePutInMessage(): (project: string, a: { id: string; title: string; url: string; agent: string }) => void {
  const router = useRouter();
  return useCallback(
    (project, a) => {
      // the composer's own drafts (app/keep.ts keptStorage), never the page's localStorage: the
      // chip never reached the composer (ui-review m_52808)
      putInDraft(keptStorage, `${project}/${a.agent}`, a);
      // and the caret after the chip, in that thread's composer (ui-review m_52822)
      askCaret(`${project}/${a.agent}`);
      router.go({ screen: 'thread', project, agent: a.agent });
    },
    [router],
  );
}

const noop = () => undefined;

// 'v' on a row: the artifact it opens shows its versions menu at once (one open, then it's spent)
let versionsFor: string | null = null;
/** The next open of this artifact ('project/id') shows its versions. */
export function openWithVersions(key: string): void {
  versionsFor = key;
}
function takeVersions(key: string): boolean {
  const on = versionsFor === key;
  versionsFor = null;
  return on;
}

// an inbox option that opens this artifact at a block (notif-open, bise-proto OptAct::Open's
// `at`): the next open of it scrolls its page there (one open, then it's spent)
let anchorFor: { key: string; at: string } | null = null;
/** The next open of this artifact ('project/id') shows it at block or item `at`. */
export function openAtAnchor(key: string, at: string): void {
  anchorFor = { key, at };
}
/** The block the view of 'project/id' opens at, read once (ArtifactOpen's first render; the edge's test reads it too). */
export function takeAnchor(key: string): string | null {
  const at = anchorFor?.key === key ? anchorFor.at : null;
  anchorFor = null;
  return at;
}

/** A frame's src at block or item `at` (the kit scrolls to its hash); none: as is. Pure. */
export function anchoredSrc(src: string | null, at: string | null): string | null {
  if (src === null || at === null || at === '') return src;
  return `${src.split('#', 1)[0] ?? ''}#${encodeURIComponent(at)}`;
}

/** ⌘C on a row: its link (the bridge's copy, else the clipboard). */
export function copyLink(url: string): void {
  const copy = bridge()?.copy;
  if (copy !== undefined) void copy(url);
  else void navigator.clipboard.writeText(url).catch(noop);
}

/** The page's url at a version: the current one as is, an older one at /v/<n>. Pure. */
export function versionUrl(url: string, current: number, v: number): string {
  if (v === current) return url;
  return `${url.replace(/\/+$/, '')}/v/${String(v)}`;
}

/** The page's url with his theme as a hint the kit reads (?theme=light|dark); none: as is. Pure. */
export function themedUrl(url: string, theme: 'light' | 'dark' | null): string {
  if (theme === null) return url;
  const [base = '', hash] = url.split('#', 2);
  const q = `${base}${base.includes('?') ? '&' : '?'}theme=${theme}`;
  return hash === undefined ? q : `${q}#${hash}`;
}

/**
 * What the open view frames at version `shown`, or null (no frame: the 'opens in your browser'
 * line, never a white box). Pure. The hub's frame_url first: a local file's copy on the pages
 * server (amb-feed c5caeffd, architect m_19561), the one origin the shell's guard lets a subframe
 * show; it names the current version only. Else a page's http(s) url (an older version at /v/<n>).
 * A bare local path from an older hub frames nothing: the guard blocks file:// (ambient-lead m_19544).
 */
export function frameSrc(
  a: { kind: string; url: string; version: number; frame_url?: string | null; versions?: readonly { v: number; frame_url?: string | null }[] },
  shown: number,
  theme: 'light' | 'dark' | null,
): string | null {
  const local = a.frame_url ?? '';
  // the newest version of an xlsx, pptx or docx: bise's viewer at /a/<id>/view, the src the frame
  // keeps while new versions land (pages::site::frame_href); never built here (architect m_23970)
  if (shown === a.version && local !== '') return themedUrl(local, theme);
  // an older version: the hub's own url for it (ArtifactVersion.frame_url, art-live 929ead94f)
  const older = shown === a.version ? '' : (a.versions?.find((x) => x.v === shown)?.frame_url ?? '');
  if (older !== '') return themedUrl(older, theme);
  if (kindOf(a.kind) !== 'page' || !/^https?:\/\//i.test(a.url)) return null;
  return themedUrl(versionUrl(a.url, a.version, shown), theme);
}

/**
 * What '↗ browser' opens at version `shown`, or null (no button). Pure. What the panel frames (the
 * hub's frame_url of that version, without the ?theme the window adds: his browser picks its own),
 * else the row's http(s) url at that version (a deploy, a link). Never a file path: the shell's
 * openExternal refuses it, so the button did nothing for a copied file (art-viewer, architect m_25527).
 */
export function browserUrl(
  a: { kind: string; url: string; version: number; frame_url?: string | null; versions?: readonly { v: number; frame_url?: string | null }[] },
  shown: number,
): string | null {
  const framed = frameSrc(a, shown, null);
  if (framed !== null) return framed;
  return /^https?:\/\//i.test(a.url) ? versionUrl(a.url, a.version, shown) : null;
}

/**
 * The head's first button, its primary action (⌘⏎, front.ts): '↗ browser' when browserUrl gives a
 * url (pages, decks, sheets, docs, links, a PR's page), else '⤓ save a copy…' when `save` is the
 * version's bytes (a file with no page); null: neither button. Pure.
 */
export type Primary = { readonly act: 'browser'; readonly url: string } | { readonly act: 'save' };
export function primaryOf(out: string | null, canSave: boolean): Primary | null {
  if (out !== null) return { act: 'browser', url: out };
  return canSave ? { act: 'save' } : null;
}

/**
 * A row's primary action (the folded float's row, ⌘⏎ under the pointer): what its head's first
 * button does at its current version, run from the row; null: none. The save goes straight to the
 * window's save sheet (no strip: the row has none).
 */
export function rowPrimary(a: Artifact): (() => void) | null {
  const bise = bridge();
  const toSave = saveOf(a, a.version);
  const p = primaryOf(browserUrl(a, a.version), toSave !== null && bise?.saveCopy !== undefined);
  if (p === null) return null;
  if (p.act === 'browser') return () => { void bise?.openExternal?.(p.url); };
  return () => { if (toSave !== null) void bise?.saveCopy?.(toSave.path, toSave.name); };
}

/** The save line icon (designer m_26482): an arrow down onto a short line, 16 px, stroke 1.5. */
function SaveIcon() {
  return (
    <svg className="art-save-ico" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M8 2.5v8M4.75 7.25 8 10.5l3.25-3.25M3 13.25h10" />
    </svg>
  );
}

/** A path's folder part with its last '/' ('~/Downloads/'), and its name ('deck.pptx'). Pure. */
export const folderOf = (p: string): string => p.slice(0, p.lastIndexOf('/') + 1);
export const nameOf = (p: string): string => p.slice(p.lastIndexOf('/') + 1);

/** A folder with its first `drop` folders cut at a slash: ('~/a/b/c/', 2) is '…/b/c/'. Pure. */
export function cutFolder(folder: string, drop: number): string {
  if (drop <= 0) return folder;
  const parts = folder.split('/');
  // the last part is '' (the folder ends with '/'): one folder always stays
  const keep = parts.slice(Math.min(drop, Math.max(parts.length - 2, 0)));
  return `…/${keep.join('/')}`;
}

/** The saved folder, cut at a slash from the left until it fits its box (designer m_26488). */
function FolderFit({ folder }: { folder: string }) {
  const at = useRef<HTMLSpanElement>(null);
  const [drop, setDrop] = useState(0);
  const most = Math.max(folder.split('/').length - 2, 0);
  // a new width: start whole again (a new folder mounts a new FolderFit: its key)
  useEffect(() => {
    const el = at.current;
    if (el === null || typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(() => { setDrop(0); });
    ro.observe(el.parentElement ?? el);
    return () => { ro.disconnect(); };
  }, [folder]);
  useLayoutEffect(() => {
    const el = at.current;
    if (el !== null && el.scrollWidth > el.clientWidth && drop < most) setDrop(drop + 1);
  }, [drop, most, folder]);
  return <span ref={at} className="art-saved-dir">{cutFolder(folder, drop)}</span>;
}

/**
 * After 'save a copy…' (art-save): 'saved to ~/Downloads/deck.pptx' with 'show in Finder', or
 * '▲ couldn't save: <why>' (▲ red); a long folder is cut from the left, the name stays whole
 * (designer m_26482), in the live strip's place; it goes when he saves again or moves.
 */
export function SavedStrip({ saved, home, onReveal }: { saved: SaveResult; home: string | undefined; onReveal?: ((to: string) => void) | undefined }) {
  if (saved === null) return null;
  if ('error' in saved) {
    return (
      <div className="art-live" data-testid="artifact.saved" data-state="error">
        <span className="art-saved-err">▲</span>
        <span className="art-saved-to">{`couldn't save: ${saved.error}`}</span>
      </div>
    );
  }
  return (
    <div className="art-live" data-testid="artifact.saved" data-state="saved">
      <span className="art-saved-to" title={saved.to}>
        <span>saved to </span>
        <FolderFit key={saved.to} folder={folderOf(tilde(saved.to, home))} />
        <span className="art-saved-name">{nameOf(saved.to)}</span>
      </span>
      {onReveal !== undefined && (
        <Press look="art-live-back" onClick={() => { onReveal(saved.to); }} data-testid="artifact.saved.finder">
          show in Finder
        </Press>
      )}
    </div>
  );
}

/**
 * The hub's live words under the head (art-live, designer m_23868, the 28 px strip): '∿ deck is
 * updating · slide 2' while the maker's turn runs, then 'v3 · 2 slides changed' with '← v2' 16 px
 * after it, which opens that version. Drawn as they come; the one thing decided here: the line goes
 * once he moved to another version (`gone` is that line).
 */
export function LiveStrip({ live, gone, onBack }: { live: ArtifactLive | null; gone: string | null; onBack: (v: number, line: string) => void }) {
  if (live === null) return null;
  if (live.strip !== undefined && live.strip !== '') {
    return (
      <div className="art-live" data-testid="artifact.live" data-state="strip">
        {live.strip}
      </div>
    );
  }
  const line = live.line;
  if (line === undefined || line === '' || line === gone) return null;
  const back = live.back;
  return (
    <div className="art-live" data-testid="artifact.live" data-state="line">
      <span>{line}</span>
      {back !== undefined && (
        <Press look="art-live-back" onClick={() => { onBack(back.v, line); }} data-testid="artifact.live.back">
          {back.text}
        </Press>
      )}
    </div>
  );
}

/**
 * The page doesn't answer (page-unreach): in place of the frame, never a blank panel. 'checking':
 * an empty box for the probe's few ms (no flash of words); else why (downWords) and 'try again',
 * except while its hub starts (it asks again by itself). A log path opens in Finder.
 */
export function PageDown({ reach, words, onAgain, onReveal }: { reach: 'checking' | 'down'; words: DownWords; onAgain: () => void; onReveal?: (path: string) => void }) {
  if (reach === 'checking') return <div className="art-down" data-testid="artifact.checking" />;
  return (
    <div className="art-down" data-testid="artifact.down" data-starting={words.starting} role="status">
      <div className="art-down-title" data-testid="artifact.down.title">{words.title}</div>
      <div className="art-down-line" data-testid="artifact.down.line">
        {words.parts.map((p, i) => {
          if ('t' in p) return <span key={i}>{p.t}</span>;
          const to = p.reveal;
          if (to === undefined || onReveal === undefined) return <code key={i} className="art-down-mono">{p.mono}</code>;
          return (
            <Press key={i} look="art-down-mono art-down-path" title="show in Finder" onClick={() => { onReveal(to); }} data-testid="artifact.down.log">
              {p.mono}
            </Press>
          );
        })}
      </div>
      {!words.starting && (
        <Press look="art-down-again" onClick={onAgain} data-testid="artifact.down.again">try again</Press>
      )}
    </div>
  );
}

/**
 * An artifact whose file is gone (the pages server's 404; designer m_52746): ds Empty centred in
 * the stage. An older version's: 'open v<newest>'; the newest's (or the only one's): ask its agent
 * about it (the artifact in that agent's composer). The head stays as it is. Pure of its props.
 */
export function FileGone({ agent, shown, newest, onNewest, onAsk }: { agent: string; shown: number; newest: number; onNewest: () => void; onAsk: () => void }) {
  const older = shown < newest;
  return (
    <div className="art-file-gone" data-testid="artifact.filegone" role="status">
      <Empty
        title={older ? panel.art_file_gone_version : panel.art_file_gone}
        action={older
          ? <Button onClick={onNewest} testId="artifact.filegone.newest">{panel.art_file_gone_open({ v: newest })}</Button>
          : <Button onClick={onAsk} testId="artifact.filegone.ask">{panel.art_file_gone_ask({ agent })}</Button>}
      >
        {panel.art_file_gone_why}
      </Empty>
    </div>
  );
}

/**
 * '↗ browser' (page-unreach): a pages-server url is asked first; one that doesn't answer opens no
 * dead tab: the panel asks again and says why. Anything else opens at once.
 */
export function openChecked(url: string, bise: (ArtBridge & { probePage?: (u: string) => Promise<'up' | 'down' | 'gone'> }) | undefined, again: () => void): void {
  const probe = bise?.probePage;
  if (!probed(url) || probe === undefined) {
    void bise?.openExternal?.(url);
    return;
  }
  void probe(url).then((r) => {
    if (r === 'up') void bise?.openExternal?.(url);
    else again();
  });
}

export interface ArtifactOpenProps {
  project: string;
  id: string;
  /** ✕ and esc: the frame's way back (the right bar as it was); none: the artifacts list */
  onClose?: () => void;
  /** the maker's composer under the page: on when the frame has no thread beside it (under 1000 px, or the centre route) */
  composer?: boolean;
  /** before the title in its head: under 1000 px the frame's '◧ <maker>' that slides the maker's thread over (amb-win) */
  lead?: ReactNode;
  /** no head: a panel tab draws its own address bar over the page (bise fait mac, app/panel/Browser.tsx) */
  bare?: boolean;
  /** the version shown, when the frame picks it (its own ArtifactVersions); absent: the head's own pick */
  version?: number;
  /** the frame's own version pick (a panel tab's): the 'file gone' state's 'open v3' goes through it; absent: the head's own pick */
  onVersion?: (v: number | undefined) => void;
}

/**
 * An artifact's versions picker (the head's 'page · v2 ⌄' menu, and a panel tab's address): every
 * version with its words and age, the one shown checked; a pick says which.
 */
export function ArtifactVersions({ a, shown, now, anchor, onPick, onClose }: {
  a: Artifact;
  shown: number;
  now: number;
  anchor: RefObject<HTMLElement | null>;
  onPick: (v: number) => void;
  onClose: () => void;
}) {
  return (
    <PickerPop anchor={anchor} align="end" onClose={onClose} testId="artifact.versions"
      groups={[{ id: 'versions', rows: versions(a, now).map((r) => ({
        id: String(r.v),
        text: `v${String(r.v)}`,
        name: <><span className="art-v">{`v${String(r.v)}`}</span><span className="art-v-words">{r.words}</span></>,
        trailing: r.when,
        current: r.v === shown,
        testId: `artifact.versions.v${String(r.v)}`,
      })) }]}
      onPick={(id) => { onPick(Number(id)); }} />
  );
}

/**
 * One artifact open (designer's artifact-open v2, amb-win's seam m_19613): it fills the box the
 * frame gives it (the right column at 55%, or the whole width under 1000 px) and reads no route.
 * Its head, 44 high: the title, 'page · v2 ⌄', '↗ browser', ⤓ save a copy (⌘S, a file's only),
 * ⋯ (save a copy, Finder, copy link, put it in a message), ✕; esc closes a menu first, then the artifact, unless a field inside has his caret.
 * The page sits 16 px in on a 3% backdrop, r 12. The screen's ONE composer is the maker's
 * (maker.ts: the project's main when the maker is archived); the frame draws it under the
 * maker's thread, so here only with `composer`.
 */
export function ArtifactOpen({ project, id, onClose, composer = true, lead, bare = false, version, onVersion }: ArtifactOpenProps) {
  const router = useRouter();
  const it = useAllArtifacts(project).find((x) => x.a.id === id);
  // 'v' on a row of the list opens it with its versions shown
  const [menu, setMenu] = useState<'versions' | 'more' | null>(() => (takeVersions(`${project}/${id}`) ? 'versions' : null));
  const closeMenu = useCallback(() => {
    setMenu(null);
  }, []);
  const versionsAt = useRef<HTMLButtonElement>(null);
  const moreAt = useRef<HTMLButtonElement>(null);
  const [at] = useState(() => takeAnchor(`${project}/${id}`));
  const [own, setV] = useState<number | null>(() => takeVersion(`${project}/${id}`));
  const v = version ?? own;
  // a link in the page that names another artifact: the panel opens it (links.ts)
  useArtifactLinks(project);
  // the live line he moved away from (another version picked): it goes; a new line shows again
  const [goneLine, setGoneLine] = useState<string | null>(null);
  // what the last 'save a copy…' did (SavedStrip); null: nothing to say
  const [saved, setSaved] = useState<SaveResult>(null);
  const [now] = useState(() => Date.now());
  const theme = themeOf(usePrefs());
  const bise = bridge();
  const store = useStore();
  const put = usePutInMessage();
  const words = useAboutWords(project, id);
  const [answering, setAnswering] = useState<Answering | null>(null);
  const agent = words?.agent ?? '';
  const agents = useAgents(project);
  const busy = turnRuns(agents.find((x) => x.name === agent)?.status);
  // the maker its composer writes to: its place, context and model in the tab and the box's row
  const maker = words === undefined ? undefined : agents.find((x) => x.name === words.agent);
  const makerPlace = useTabPlace(project, maker, false);
  const onStop = useCallback(() => {
    store.stop(project, agent);
  }, [store, project, agent]);
  const about = useMemo(() => (it === undefined ? undefined : { id: it.a.id, title: it.a.title, url: it.a.url }), [it]);
  // the frame's src, from the hub's row (hooks before the early return), at the block an inbox
  // option opened it at, on its current version only
  const src = it === undefined ? null : anchoredSrc(frameSrc(it.a, v ?? it.a.version, theme), (v ?? it.a.version) === it.a.version ? at : null);
  // ⌘S (File › Save a Copy…): the version shown, when it has bytes to save
  const toSave = it === undefined ? null : saveOf(it.a, v ?? it.a.version);
  const save = () => {
    const doSave = bise?.saveCopy;
    if (toSave === null || doSave === undefined) return;
    setMenu(null);
    void doSave(toSave.path, toSave.name).then((r) => {
      if (r !== null) setSaved(r);
    });
  };
  useMenuAction('save', save);
  const frame = useRef<HTMLIFrameElement>(null);
  // a video's player asks for the height that holds it: the card hugs the screen and its bar (designer m_32762)
  const hug = useFrameNote(frame, src, project, id, it?.a.title ?? '');
  // page-unreach: the page is framed once its server answers; meanwhile the panel says why (reach.ts)
  const { reach, again } = useReach(src);
  // ⌘⏎ (front.ts): the head's first button, whether the head shows (the open view) or a panel tab's bar does
  const primary = it === undefined ? null : primaryOf(browserUrl(it.a, v ?? it.a.version), toSave !== null && bise?.saveCopy !== undefined);
  const runPrimary = primary === null ? null : primary.act === 'browser' ? () => { openChecked(primary.url, bise, again); } : save;
  const box = useRef<HTMLElement>(null);
  useArtifactFront(box, bare ? 'panel' : 'view', runPrimary);
  // ⌘F (find.ts): a page with words gets its find bar; null: closed, else bumped by each ⌘F
  const [findAsk, setFindAsk] = useState<number | null>(null);
  const canFind = it !== undefined && src !== null && reach === 'up' && findable(it.a);
  useArtifactFind(box, canFind ? () => { setFindAsk((x) => (x ?? 0) + 1); } : null);
  // ⌘ held: the first button's ⌘⏎ keycap in place of its words (held.ts)
  const cmd = useModHeld('Meta');
  const row = useProjects().find((p) => p.project === project);
  const close = () => {
    // ✕ and esc as ‹: back to the place before (designer's desktop-history); opened straight (a link) the list
    if (onClose !== undefined) onClose();
    else if (router.canBack()) router.back();
    else router.go({ screen: 'artifacts', project: '' });
  };
  const onKeyDown = (e: KeyboardEvent<HTMLElement>) => {
    if (e.key !== 'Escape' || e.defaultPrevented) return;
    if (menu !== null) {
      e.preventDefault();
      setMenu(null);
      return;
    }
    // his caret in a field inside (the composer): the field's esc, never a close
    const t = e.target;
    if (t instanceof HTMLElement && t.closest('input, textarea, [contenteditable="true"]') !== null) return;
    e.preventDefault();
    close();
  };
  const x = (
    <IconButton label="close" keys="esc" size="bar" onClick={close} testId="artifact.close">
      ✕
    </IconButton>
  );
  if (it === undefined) {
    // its list came without it: say so, at the content's edge, with a way on (artifacts/gone.ts);
    // before the list came, nothing yet
    const came = store.artifactsCame(project);
    const link = goneLinkOf(store.artifacts(project));
    return (
      <section className="art-open" data-testid="artifact.open" data-bare={bare || undefined} onKeyDown={onKeyDown}>
        {!bare && (
          <div className="art-open-head">
            <span className="art-open-title" />
            <span className="art-open-end">{x}</span>
          </div>
        )}
        {came && (
          <div className="art-gone" data-testid="artifact.missing">
            <p className="art-gone-line">{panel.art_gone}</p>
            <p className="art-gone-on">
              {link === 'archived' && `${panel.art_gone_maybe} · `}
              <Press look="art-gone-link" onClick={() => { router.go(goneRoute(project, link)); }} data-testid="artifact.missing.link">
                {link === 'archived' ? panel.art_gone_archived : panel.art_gone_library}
              </Press>
            </p>
          </div>
        )}
      </section>
    );
  }
  const a = it.a;
  const shown = v ?? a.version;
  const url = versionUrl(a.url, a.version, shown);
  // '↗ browser': what the panel frames, else an http(s) url; none: no button
  const out = primary?.act === 'browser' ? primary.url : null;
  // show in Finder: the file, else bise's copy when the file is gone (an archived agent's temp folder)
  const path = revealPathOf(a, shown);
  const finder = bise?.showInFinder;
  const canSave = toSave !== null && bise?.saveCopy !== undefined;
  const savePrimary = primary?.act === 'save';
  return (
    <section ref={box} className="art-open" data-testid="artifact.open" data-agent={a.agent} data-bare={bare || undefined} onKeyDown={onKeyDown}>
      {!bare && <div className="art-open-head" data-testid="artifact.head">
        {lead}
        <span className="art-open-title" data-testid="artifact.title" title={a.title}>{a.title}</span>
        <span className="art-open-end">
          <PickerTrigger ref={versionsAt} open={menu === 'versions'} onClick={() => { setMenu(menu === 'versions' ? null : 'versions'); }} testId="artifact.version" tip={shown === a.version ? 'the current version' : 'an older version'}>
            {`${kindOf(a.kind)} · v${String(shown)}`}
          </PickerTrigger>
          {menu === 'versions' && (
            <ArtifactVersions a={a} shown={shown} now={now} anchor={versionsAt} onClose={closeMenu}
              onPick={(n) => { setV(n); setMenu(null); setGoneLine(a.live?.line ?? null); setSaved(null); }} />
          )}
          {out !== null && (
            <Button kind="ghost" size="sm" onClick={() => { openChecked(out, bise, again); }} testId="artifact.browser" title={frameWords.tip_key({ what: panel.addr_out, key: PRIMARY_KEY })}>
              {cmd ? <Kbd>{PRIMARY_KEY}</Kbd> : '↗ browser'}
            </Button>
          )}
          {canSave && (
            <IconButton label={library.head_save} keys={savePrimary ? PRIMARY_KEY : '⌘S'} size="bar" onClick={save} testId="artifact.save">
              {savePrimary && cmd ? <Kbd>{PRIMARY_KEY}</Kbd> : <SaveIcon />}
            </IconButton>
          )}
          <IconButton ref={moreAt} label="more" size="bar" pressed={menu === 'more'} onClick={() => { setMenu(menu === 'more' ? null : 'more'); }} testId="artifact.more">⋯</IconButton>
          {menu === 'more' && (
            <PickerPop anchor={moreAt} align="end" onClose={closeMenu} testId="artifact.more.menu"
              groups={[{ id: 'more', rows: [
                ...(canSave ? [{ id: 'save', name: 'save a copy…', trailing: '⌘S', testId: 'artifact.save.menu' }] : []),
                ...(finder !== undefined && path !== null ? [{ id: 'finder', name: 'show in Finder', testId: 'artifact.finder' }] : []),
                { id: 'copy', name: 'copy link', testId: 'artifact.copy' },
                { id: 'put', name: 'put it in a message', testId: 'artifact.put' },
              ] }]}
              onPick={(id) => {
                setMenu(null);
                if (id === 'save') save();
                else if (id === 'finder' && finder !== undefined && path !== null) void finder(path);
                else if (id === 'copy') void bise?.copy?.(url);
                else if (id === 'put') put(project, a);
              }} />
          )}
          {x}
        </span>
      </div>}
      <LiveStrip live={a.live ?? null} gone={goneLine} onBack={(back, line) => { setV(back); setGoneLine(line); setSaved(null); }} />
      <SavedStrip saved={saved} home={bise?.home} onReveal={finder === undefined ? undefined : (to) => { void finder(to); }} />
      <div className="art-open-body">
        {/* the page follows his theme: opened with ?theme=<his pref>, the kit wears it (kit.js puts
            it on <html data-theme>); with no pref, the system's. A browser-only kind never gets
            here from a list (leave.ts opens his browser): no 'opens in your browser' panel (designer m_21714) */}
        {/* the src stays /a/<id>/view while new versions land: the viewer swaps in place, never a
            reload (plan §6). Its name tells the viewer it sits in the app: a selection comes here
            (useFrameNote) for the maker's composer, not the frame's own note. An image zooms in
            the viewer itself (kit/office/image.js; image-pinch), its esc and ⌘0 lent by useFrameNote.
            A video's player goes full screen from its own button (kit/office/video.js): allow it */}
        {/* its file gone (the server's 404, designer m_52746): bise's own empty state, never the server's line */}
        {src !== null && reach === 'gone' && (
          <FileGone agent={a.agent} shown={shown} newest={a.version}
            onNewest={() => {
              if (onVersion !== undefined) onVersion(undefined);
              else setV(null);
              setGoneLine(null);
              setSaved(null);
            }}
            onAsk={() => { put(project, a); }} />
        )}
        {src !== null && (reach === 'checking' || reach === 'down') && (
          <PageDown reach={reach} words={downWords(row === undefined ? undefined : { name: row.name, running: row.running, down: row.down ?? null }, src, bise?.home)}
            {...(finder === undefined ? {} : { onReveal: (p: string) => { void finder(p); } })}
            onAgain={() => {
              if (store.hubRefused(project) !== null) store.hubRetry(project);
              again();
            }} />
        )}
        {src !== null && reach === 'up' && (
          <iframe ref={frame} name={APP_FRAME_NAME} className="art-paper" src={src} title={a.title} sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
            allow="fullscreen" style={hug === null ? undefined : { maxHeight: `${String(hug)}px` }}
            data-testid="artifact.page" />
        )}
        {/* ⌘F's bar over the page's top right (find.ts, FindBar.tsx); a new src starts it again */}
        {canFind && findAsk !== null && (
          <ArtifactFindBar key={src} frame={frame} src={src} ask={findAsk} onClose={() => {
            setFindAsk(null);
            frame.current?.focus();
          }} />
        )}

      </div>
      {/* the screen's one composer, the maker's (maker.ts), when no thread sits beside the page; the
          maker's own question docks over it (designer m_20452: only the maker's, as in its thread) */}
      {composer && words !== undefined && (
        <div className="art-open-composer th-screen" data-testid="artifact.composer" data-answering={answering !== null}>
          <InboxBlock here={{ project, agent: a.agent }} only={(c) => c.project === project && c.agent === a.agent} onAnswer={setAnswering} />
          <ThreadComposer
            project={project}
            agent={words.agent}
            busy={busy}
            // the crumb names the project here: the tab says who and where (designer's composer-strip A)
            tab={{ label: answering?.label ?? words.to, place: makerPlace, context: usageLine(maker?.usage?.words), who: words.agent }}
            tabEnd={answering !== null ? answering.end : words.end}
            me={maker}
            aboutPink={words.pink?.tip}
            placeholder={answering?.placeholder ?? words.placeholder}
            answer={answerOf(answering)}
            onStop={onStop}
            onReady={noop}
            onSent={noop}
            {...(about === undefined ? {} : { about })}
          />
        </div>
      )}
    </section>
  );
}
