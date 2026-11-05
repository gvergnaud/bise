// The @ $ / list above the composer: the ds Picker (designer's list-menu m_24793, m_25011), one
// 32 px row per entry: its kind's glyph (only when the list mixes kinds), its name, its description
// (or a file's folder) inline. The @ list (at-menu-icons, designer's C): its rows under the heads
// Agents, Plugins, Artifacts, Files (a group only with rows; never a row, so the keys skip it), each
// with its 16 px line icon in text-3: an agent the thread bubble, a plugin the plug, an artifact its
// kind's icon (a folder of them the stack), a file and a folder theirs; the $ and / lists keep '$'
// and '/'. The editor keeps the keys: the keyboard's row is `active`; a greyed row reads `not yet`.
// A click picks. It floats on <body>, never inside the composer's box (its overflow is hidden, and
// the floating composer's too: Gabriel m_44279 saw them cut): 8 px above the box, below when the
// room above is short, its height the room less 24, the list scrolling inside (pickPlace). Placed
// when it opens, again on a window resize or the box's own resize (his lines growing it), never per
// keystroke. Owner: r2-hints (nm-composer's before).
import { useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type JSX } from 'react';
import { createPortal } from 'react-dom';
import { AppIcon, iconKindOf, KindIcon, PICKER_CAP, Picker as DsPicker, type PickerGroup, type PickerRow } from '@/ds';
import { composer } from '../proto/words.gen';
import type { Row } from './pick';

/** The room a list needs to open on the box's side it prefers (above), else it opens on the larger one. */
const MIN_ROOM = 240;
/** Between the list and the box. */
const GAP = 8;
/** Kept free between the list and the window's edge (r2-hints item 2). */
const EDGE = 24;

/** Where the @ $ / list floats from the composer's box: its left edge on the box's, 8 px above it
 *  (below when the room above is under 240 px and below is larger), its whole height the room on
 *  that side less 24, and at most half the window; fixed coordinates, inside the window. Pure. */
export function pickPlace(
  box: { top: number; bottom: number; left: number },
  win: { width: number; height: number },
): { style: CSSProperties; maxHeight: number; up: boolean } {
  const above = box.top - GAP - EDGE;
  const below = win.height - box.bottom - GAP - EDGE;
  const up = above >= MIN_ROOM || above >= below;
  const left = Math.max(GAP, Math.min(box.left, win.width - GAP - MIN_ROOM));
  const y: CSSProperties = up ? { bottom: win.height - box.top + GAP } : { top: box.bottom + GAP };
  return { style: { position: 'fixed', left, ...y }, maxHeight: Math.max(0, Math.min(Math.floor(win.height * PICKER_CAP), up ? above : below)), up };
}

/** A row's words on the ds Picker: the kind's glyph out of the label, a file's folder under its name. Pure. */
export function pickerRow(r: Row, testId: string): PickerRow {
  const base = {
    id: r.id,
    disabled: r.off,
    testId: `${testId}.${r.id}`,
    tip: r.tip ?? (r.desc === '' ? undefined : `${r.label} · ${r.desc}`),
    ...(r.current === true ? { current: true } : {}),
  };
  const sec = r.desc === '' ? undefined : r.desc;
  switch (r.tag) {
    case 'cmd':
      return { ...base, glyph: '/', name: r.label.replace(/^\//u, ''), secondary: sec };
    case 'skill':
      return { ...base, glyph: '$', name: r.label.replace(/^\$/u, ''), secondary: sec };
    case 'agent':
      return { ...base, glyph: <AppIcon name="thread" />, glyphKind: 'agent', name: r.label.replace(/^@/u, ''), secondary: sec };
    case 'artifact':
      return { ...base, glyph: <ArtifactIcon what={r.pick.kind === 'artifact' ? r.pick.what : ''} url={r.pick.kind === 'artifact' ? r.pick.url : ''} />, glyphKind: 'artifact', name: r.label.replace(/^↗\s*/u, ''), secondary: sec };
    case 'plugin': {
      // the plug (designer's C); a waiting one's mark in color: '?' accent, '✗' err (m_53299)
      const name = r.label.replace(/^@/u, '');
      const plug = { glyph: <AppIcon name="plugins" />, glyphKind: 'plugin' };
      if (r.waits === undefined || sec === undefined) return { ...base, ...plug, name, secondary: sec };
      const mark = r.waits === 'login' ? '?' : '✗';
      const at = sec.indexOf(`${mark} `);
      const secondary =
        at < 0 ? (
          sec
        ) : (
          <span>
            {sec.slice(0, at)}
            <span className={r.waits === 'login' ? 'at-plugin-wait' : 'at-plugin-fail'}>{mark}</span>
            {sec.slice(at + 1)}
          </span>
        );
      return { ...base, ...plug, name, secondary };
    }
    case 'file':
    case 'dir': {
      const path = r.label.replace(/\/$/u, '');
      const cut = path.lastIndexOf('/');
      const name = (cut < 0 ? path : path.slice(cut + 1)) + (r.tag === 'dir' ? '/' : '');
      const folder = cut < 0 ? '' : path.slice(0, cut);
      const icon = r.tag === 'dir' ? { glyph: <AppIcon name="folder" />, glyphKind: 'dir' } : { glyph: <AppIcon name="file" />, glyphKind: 'file' };
      return { ...base, ...icon, name, secondary: folder === '' ? undefined : <span className="b-picker-path">{folder}</span>, tip: r.label };
    }
    default:
      return { ...base, name: r.label, secondary: sec };
  }
}

/** An artifact's icon (the @ list's row, its chip's the same): its kind's, a folder of artifacts the
 *  'artifacts' stack. */
function ArtifactIcon({ what, url }: { what: string; url: string }): JSX.Element {
  return what.toLowerCase() === 'folder' ? <AppIcon name="artifacts" /> : <KindIcon kind={iconKindOf({ kind: what, url })} />;
}

/** The @ list's groups, in their order, each with its head (designer's C). */
const AT_GROUPS = [
  { id: 'agents', head: composer.at_group_agents, tags: ['agent'] },
  { id: 'plugins', head: composer.at_group_plugins, tags: ['plugin'] },
  { id: 'artifacts', head: composer.at_group_artifacts, tags: ['artifact'] },
  { id: 'files', head: composer.at_group_files, tags: ['file', 'dir'] },
] as const;

/** The list's groups: the @ list's rows under their heads (Agents, Plugins, Artifacts, Files, in that
 *  order, a group only with rows, today's order inside), any other list one group with no head. The
 *  heads are no rows: the keys (the editor's index over `rows`) never land on one. Pure. */
export function pickerGroups(rows: readonly Row[], testId: string): PickerGroup[] {
  const at = rows.some((r) => AT_GROUPS.some((g) => (g.tags as readonly string[]).includes(r.tag)));
  if (!at) return [{ id: 'rows', rows: rows.map((r) => pickerRow(r, testId)) }];
  return AT_GROUPS.map((g) => ({
    id: g.id,
    head: g.head,
    look: 'at-group',
    rows: rows.filter((r) => (g.tags as readonly string[]).includes(r.tag)).map((r) => pickerRow(r, testId)),
  })).filter((g) => g.rows.length > 0);
}

/** `empty`: no rows, its one text-3 line instead (a bare '@' or '$' with nothing to offer) */
export function Picker({ rows, index, testId, onPick, empty }: { rows: readonly Row[]; index: number; testId: string; onPick: (i: number) => void; empty?: string }): JSX.Element {
  // where it is in the composer: its box (the frame's .box), measured from here
  const at = useRef<HTMLSpanElement>(null);
  const [place, setPlace] = useState<ReturnType<typeof pickPlace> | null>(null);
  useLayoutEffect(() => {
    // the box it floats from: a frame around the composer that says so (the floating composer's
    // glass, data-pick-anchor), else the composer's own box
    const box = at.current?.closest('[data-pick-anchor]') ?? at.current?.closest('.box') ?? at.current?.parentElement ?? null;
    if (box === null) return;
    const measure = () => {
      const next = pickPlace(box.getBoundingClientRect(), { width: window.innerWidth, height: window.innerHeight });
      setPlace((p) => (p !== null && JSON.stringify(p) === JSON.stringify(next) ? p : next));
    };
    measure();
    window.addEventListener('resize', measure);
    const ro = typeof ResizeObserver === 'function' ? new ResizeObserver(measure) : null;
    ro?.observe(box);
    return () => {
      window.removeEventListener('resize', measure);
      ro?.disconnect();
    };
  }, []);
  const groups = useMemo(() => pickerGroups(rows, testId), [rows, testId]);
  return (
    <span ref={at} className="b-composer-pick-at" hidden>
      {createPortal(
        <div className="b-composer-pick" style={place?.style} data-up={place?.up === false ? 'false' : undefined}>
          <DsPicker
            groups={groups}
            active={rows[index]?.id ?? null}
            onPick={(id) => {
              const i = rows.findIndex((r) => r.id === id);
              if (i >= 0) onPick(i);
            }}
            maxHeight={place?.maxHeight}
            empty={empty}
            inline
            testId={testId}
          />
        </div>,
        document.body,
      )}
    </span>
  );
}
