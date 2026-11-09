// An artifact pill's hover preview in a thread (Gabriel m_58167, designer m_58173): resting on a
// pill (or tabbing to it) shows a small card above it, the artifact's cached picture, its title and
// 'page · designer · 2 h ago'; it goes when the pointer leaves the pill and the card, on esc, a
// press, a scroll or the window's blur. A click on the pill still opens the artifact (ArtName's
// own); a click on the card opens it too.
//
// The card is never a blank box: its picture is the gallery card's, from the one thumb queue
// (art-thumbs), asked when the pointer enters the pill, so the 150 ms rest has made it; one in the
// queue's memory draws at once with no ask; until it comes, the kind's faint icon. A thread that
// scrolls under a resting pointer shows nothing until the pointer itself moves.
//
// The thread provides where a pill's artifact lives and how it opens (ArtPeekContext); with none
// (an inbox line, the agents' peek) the pill is drawn alone. When and where: peek.ts (pure).
// Owner: artifact-peek.
import { createContext, useCallback, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { KindIcon, iconKindOf } from '@/ds';
import type { Artifact } from '@/proto';
import { useStore, type Store } from '@/store';
import { Thumb } from '@/artifacts/thumbs';
import { planOf } from '@/artifacts/thumbs/plan';
import { thumbQueue } from '@/artifacts/thumbs/run';
import type { Item } from '@/artifacts/model';
import { PEEK_GONE, PEEK_GRACE_MS, PEEK_WARM_MS, PEEK_WIDTH, peekMeta, peekWait, placePeek } from './peek';
import './peek.css';

/** Where a thread's pills' artifacts live and how one opens. */
export interface PeekSource {
  readonly project: string;
  readonly open: (id: string) => void;
}
export const ArtPeekContext = createContext<PeekSource | undefined>(undefined);

// the window's one preview: a new pill's closes the last one's; warm for a second after one closes
let closeShown: (() => void) | null = null;
let shownBy: object | null = null;
let warmUntil = 0;
// the pointer really moved (not a thread scrolled under a still pointer, designer m_58173)
let lastX = Number.NaN;
let lastY = Number.NaN;
let moved = false;
let tracking = false;
function track(): void {
  if (tracking || typeof document === 'undefined') return;
  tracking = true;
  document.addEventListener(
    'pointermove',
    (e) => {
      moved = e.clientX !== lastX || e.clientY !== lastY;
      lastX = e.clientX;
      lastY = e.clientY;
    },
    true,
  );
}

/** Tests: a fresh window (not warm, no preview, the pointer unseen). */
export function resetPeek(): void {
  closeShown = null;
  shownBy = null;
  warmUntil = 0;
  lastX = Number.NaN;
  lastY = Number.NaN;
  moved = false;
}

const itemOf = (project: string, a: Artifact): Item => ({ project, projectName: '', a });
const find = (store: Store, project: string, id: string): Artifact | undefined => store.artifacts(project).find((x) => x.id === id);

/** Its picture asked now, so the rest before the card has made it (one already here: no ask). */
function warm(store: Store, project: string, id: string): void {
  const a = find(store, project, id);
  const q = thumbQueue();
  if (a === undefined || q === null) return;
  const p = planOf(itemOf(project, a));
  if (p.how === 'file' || p.how === 'page') void q.get({ how: p.how, src: p.src, project: p.project, id: p.id, v: p.v, sig: p.sig }, 'view');
}

const focusVisible = (el: Element): boolean => {
  try {
    return el.matches(':focus-visible');
  } catch {
    return false;
  }
};

/** A pill with its preview: `name` the pill's words (the title of one gone from the store). */
export function ArtPeek({ id, name, children }: { id: string; name: string; children: ReactNode }) {
  const src = useContext(ArtPeekContext);
  if (src === undefined) return <>{children}</>;
  return (
    <PeekHost id={id} name={name} src={src}>
      {children}
    </PeekHost>
  );
}

interface Open {
  /** shown cold (after the rest): it scales in; a warm one fades only */
  readonly cold: boolean;
  /** when it showed: its age words are of then */
  readonly now: number;
  /** the pointer's height when it showed (a pill on two lines: the line under it), else none */
  readonly y?: number;
}

function PeekHost({ id, name, src, children }: { id: string; name: string; src: PeekSource; children: ReactNode }) {
  const store = useStore();
  const pillRef = useRef<HTMLSpanElement>(null);
  const [open, setOpen] = useState<Open | null>(null);
  // this pill, as the window's one preview knows it
  const me = useRef({});
  const showT = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const hideT = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const clear = () => {
    clearTimeout(showT.current);
    clearTimeout(hideT.current);
    showT.current = undefined;
    hideT.current = undefined;
  };
  const close = useCallback(() => {
    clearTimeout(showT.current);
    clearTimeout(hideT.current);
    showT.current = undefined;
    hideT.current = undefined;
    setOpen((o) => {
      if (o !== null) warmUntil = Date.now() + PEEK_WARM_MS;
      return null;
    });
    if (shownBy === me.current) {
      shownBy = null;
      closeShown = null;
    }
  }, []);
  const show = (o: Omit<Open, 'now'>) => {
    if (closeShown !== null && shownBy !== me.current) closeShown();
    shownBy = me.current;
    closeShown = close;
    setOpen({ ...o, now: Date.now() });
  };
  const arm = (focus: boolean, y?: number) => {
    clearTimeout(hideT.current);
    hideT.current = undefined;
    if (open !== null || showT.current !== undefined) return;
    warm(store, src.project, id);
    const wait = peekWait(Date.now(), warmUntil, focus);
    const o = { cold: wait > 0, ...(y === undefined ? {} : { y }) };
    if (wait === 0) show(o);
    else
      showT.current = setTimeout(() => {
        showT.current = undefined;
        show(o);
      }, wait);
  };
  const leave = () => {
    clearTimeout(showT.current);
    showT.current = undefined;
    if (open === null) return;
    clearTimeout(hideT.current);
    hideT.current = setTimeout(close, PEEK_GRACE_MS);
  };
  useEffect(() => {
    track();
    return clear;
  }, []);
  // while it shows: esc, a press out of the card, a scroll, the window's blur close it
  useEffect(() => {
    if (open === null) return;
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close();
    };
    const press = (e: PointerEvent) => {
      if (!(e.target instanceof Element && e.target.closest('[data-peek-card]') !== null)) close();
    };
    document.addEventListener('keydown', key, true);
    document.addEventListener('pointerdown', press, true);
    document.addEventListener('scroll', close, true);
    window.addEventListener('blur', close);
    return () => {
      document.removeEventListener('keydown', key, true);
      document.removeEventListener('pointerdown', press, true);
      document.removeEventListener('scroll', close, true);
      window.removeEventListener('blur', close);
    };
  }, [open, close]);
  useEffect(() => close, [close]);
  return (
    <span
      ref={pillRef}
      className="ap-pill"
      onPointerMove={(e) => {
        if (moved) arm(false, e.clientY);
      }}
      onPointerLeave={leave}
      onFocus={(e) => {
        if (focusVisible(e.target)) arm(true);
      }}
      onBlur={close}
      data-testid="thread.peek.pill"
    >
      {children}
      {open !== null && (
        <PeekCard
          id={id}
          name={name}
          src={src}
          open={open}
          pill={pillRef}
          onEnter={() => {
            clearTimeout(hideT.current);
            hideT.current = undefined;
          }}
          onLeave={leave}
          onOpen={() => {
            close();
            src.open(id);
          }}
        />
      )}
    </span>
  );
}

/** The pill's box to place against: its line under the pointer (a pill wrapped on two lines), else its first. */
function pillBox(el: HTMLElement, y: number | undefined): DOMRect {
  const rects = Array.from(el.getClientRects());
  const at = y === undefined ? undefined : rects.find((r) => y >= r.top && y <= r.bottom);
  return at ?? rects[0] ?? el.getBoundingClientRect();
}

function PeekCard({ id, name, src, open, pill, onEnter, onLeave, onOpen }: {
  id: string;
  name: string;
  src: PeekSource;
  open: Open;
  pill: React.RefObject<HTMLSpanElement | null>;
  onEnter: () => void;
  onLeave: () => void;
  onOpen: () => void;
}) {
  const store = useStore();
  const a = useSyncExternalStore(
    (cb) => store.subscribe(`artifacts:${src.project}`, cb),
    () => find(store, src.project, id),
  );
  const item = useMemo(() => (a === undefined ? undefined : itemOf(src.project, a)), [a, src.project]);
  const plan = useMemo(() => (item === undefined ? undefined : planOf(item)), [item]);
  const ref = useRef<HTMLDivElement>(null);
  // placed once its size is known, before the paint: above first, its left edge on the pill's
  useLayoutEffect(() => {
    const el = ref.current;
    const p = pill.current;
    if (el === null || p === null) return;
    const m = pillBox(p, open.y);
    const at = placePeek({ left: m.left, top: m.top, width: m.width, height: m.height }, { width: el.offsetWidth, height: el.offsetHeight }, { width: window.innerWidth, height: window.innerHeight });
    el.style.left = `${String(at.left)}px`;
    el.style.top = `${String(at.top)}px`;
    el.style.transformOrigin = `${String(Math.round(m.left - at.left + Math.min(m.width, 24) / 2))}px ${at.side === 'above' ? '100%' : '0'}`;
    el.dataset.side = at.side;
    el.style.visibility = 'visible';
  });
  const title = a !== undefined && a.title !== '' ? a.title : name;
  return createPortal(
    <div
      ref={ref}
      className={open.cold ? 'ap-card cold' : 'ap-card'}
      style={{ width: PEEK_WIDTH, visibility: 'hidden' }}
      role="link"
      aria-label={title}
      onPointerEnter={onEnter}
      onPointerLeave={onLeave}
      onClick={(e) => {
        e.stopPropagation();
        onOpen();
      }}
      data-peek-card=""
      data-kind={plan?.how ?? 'gone'}
      data-testid="thread.peek"
    >
      {item !== undefined && plan?.how === 'tile' && (
        <div className="ap-tile" data-testid="thread.peek.tile">
          <KindIcon kind={plan.icon === 'link' ? iconKindOf(item.a) : plan.icon} size={16} />
          <span className="ap-tile-w">{plan.words}</span>
        </div>
      )}
      {item !== undefined && plan?.how !== 'tile' && (
        <div className="ap-frame" data-testid="thread.peek.frame">
          <Thumb item={item} visible />
        </div>
      )}
      <div className="ap-title" data-testid="thread.peek.title">
        {title}
      </div>
      <div className={a === undefined ? 'ap-meta gone' : 'ap-meta'} data-testid="thread.peek.meta">
        {a === undefined ? PEEK_GONE : peekMeta(a, open.now)}
      </div>
    </div>,
    document.body,
  );
}
