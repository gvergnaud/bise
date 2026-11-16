// src/app/side3d · a bubble's float (page native-mac §7, mockups 12, 13; one per bubble, designer
// m_44975): 360 wide, 12 left of the bubbles, its top on its bubble's, at most the room under it
// less 16 and 560 high (layout cardMaxH), r 16, glass (--c-overlay + --glass), --e-3. One frame
// (Float) that stays mounted from one bubble to another (designer m_44990: Host moves it), its
// content a layer (FloatLayer): a head of 32 (its name 13/600, what it counts text-3, then '+' and
// '▯' open as a tab), its rows (they scroll inside under a 16 fade when they overflow; the head
// and the foot stay whole), one foot: a rule and a 32 row of 12 text-3 ('3 archived' · 'all
// agents ›'; every float's 'all … ›' is there, never on its head, designer m_44411), the float's
// end. While it moves, the leaving bubble's layer fades out over the new one, inert. Floats over
// the page with its whole shadow (SideHost is fixed at the shell's root: noClip). Its rows:
// SideRow, 48 high, a 40 object, 12 between, a name and a line under it, a dot at the end; its
// button is a `data-row` (SideHost's ↑ ↓ walk the rows, ⏎ opens). Esc and a click outside close
// it (SideHost). Owner: nm-side; the frame and its move: m-bubbles.
import { useEffect, useRef, useState, type MouseEventHandler, type ReactNode, type Ref, type RefObject } from 'react';
import { AppIcon, Press, tipAttrs } from '@/ds';
import type { EntityMenu } from '@/ds';
import type { BubbleId } from './objects';
import { PinButton, PinMark, type PinOf } from '../pinbutton';

/** A card's head button (+, ▯): 28, its line icon at 16, its tip and VoiceOver name. */
export function HeadButton({ icon, tip, onClick, testId }: { icon: 'plus' | 'panel'; tip: string; onClick: () => void; testId: string }) {
  return (
    <Press look="s3-icon-btn" aria-label={tip} {...tipAttrs(tip)} onClick={onClick} data-testid={testId}>
      <AppIcon name={icon} size={16} />
    </Press>
  );
}

/** A card's link ('all agents ›'). */
export function SideLink({ children, onClick, testId }: { children: ReactNode; onClick: () => void; testId: string }) {
  return (
    <Press look="s3-link" onClick={onClick} data-testid={testId}>
      {children}
    </Press>
  );
}

/** A foot's link to a full page. */
export interface FootLink {
  readonly text: string;
  readonly onClick: () => void;
  readonly testId: string;
}

/** What a bubble's float shows: its head, its rows, its foot. */
export interface FloatContent {
  readonly id: BubbleId;
  /** its name ('Agents') and what it counts ('3 working · 1 asks') */
  readonly name: string;
  readonly says: string;
  /** the part of its head that waits on him, after `says`, in the accent ('1 waiting for you', page agents-float v1) */
  readonly loud?: string;
  /** the head's '+' (agents: a new agent; artifacts: add a file) */
  readonly add?: ReactNode;
  /** its foot: what it says on the left ('3 archived'), its full page's link on the right ('all agents ›') */
  /** its foot: what it says on the left ('3 archived'), or a fold there (the agents' '▸ 3 archived'); its full page's link on the right */
  readonly foot: { readonly left: string; readonly fold?: ReactNode; readonly links: readonly FootLink[] };
  readonly rows: ReactNode;
  /** a block between its rows and its foot, out of their scroll box: nothing scrolls under it (the
   *  agents card's Tasks, designer m_65917) */
  readonly below?: ReactNode;
  /** a line under its head: its scope on a sub-agent's thread ('by perf', the Overview's, folded-side v9) */
  readonly scope?: { readonly words: string; readonly tip: string };
}

/**
 * One float's head, rows and foot: a layer of the frame (Float). `leaving`: the content of the
 * bubble it moves away from, fading out over the new one while the frame moves (designer m_44990);
 * inert, out of VoiceOver and the rows' walk.
 */
export function FloatLayer({ c, scrolls = false, leaving = false }: { c: FloatContent; scrolls?: boolean; leaving?: boolean }) {
  const id = c.id;
  return (
    <div className="s3-card-layer" data-leaving={leaving || undefined} data-now={!leaving || undefined} aria-hidden={leaving || undefined}
      inert={leaving || undefined} data-testid={leaving ? `app.flyout.${id}.leaving` : undefined}>
      <header className="s3-card-head">
        <b>{c.name}</b>
        {c.says !== '' && <span className="s3-card-says">{c.says}</span>}
        {c.loud !== undefined && c.loud !== '' && (
          <span className="s3-card-says">
            {c.says !== '' && '· '}
            <span className="s3-card-says" data-loud="" data-testid={`app.flyout.${id}.loud`}>{c.loud}</span>
          </span>
        )}
        <span className="s3-card-acts">
          {c.add}
        </span>
      </header>
      {c.scope !== undefined && (
        <div className="s3-card-scope" {...tipAttrs(c.scope.tip)} data-testid={`app.flyout.${id}.scope`}>
          {c.scope.words}
        </div>
      )}
      <div className="s3-card-body" data-scrolls={scrolls || undefined}>
        {c.rows}
      </div>
      {c.below}
      <footer className="s3-card-foot">
        <span className="s3-card-foot-left">{c.foot.fold ?? c.foot.left}</span>
        <span className="s3-card-links">
          {c.foot.links.map((l) => (
            <SideLink key={l.testId} onClick={l.onClick} testId={l.testId}>
              {l.text}
            </SideLink>
          ))}
        </span>
      </footer>
    </div>
  );
}

export interface FloatProps {
  readonly id: BubbleId;
  readonly name: string;
  /** its top, px from the side's top (its bubble's) */
  readonly top: number;
  /** its height at most (layout cardMaxH: the room under its bubble less 16, 560 at most); its rows scroll inside */
  readonly maxH: number;
  /** it moves from one bubble to another: its content clipped to it, the two layers crossfading */
  readonly moving: boolean;
  readonly frameRef: Ref<HTMLElement>;
  /** its layer now, then the leaving one while it moves */
  readonly children: ReactNode;
  /** a click in it, before its row's own act and after it (SideHost: a row that navigates closes it) */
  readonly onClicked?: { readonly before: () => void; readonly after: () => void };
}

/**
 * The one float (designer m_44975, m_44990): a frame that stays mounted from one bubble to
 * another; Host moves it (its top and height, 240 ms) and swaps its layers.
 */
export function Float({ id, name, top, maxH, moving, frameRef, children, onClicked }: FloatProps) {
  return (
    <section ref={frameRef} className="s3-card" style={{ top, maxHeight: maxH }} data-foot="" data-moving={moving || undefined} data-tip-box=""
      role="dialog" aria-label={name} data-testid={`app.flyout.${id}`} onClickCapture={onClicked?.before} onClick={onClicked?.after}>
      {children}
    </section>
  );
}

export interface SideRowProps {
  /** the 40 object */
  readonly pic: ReactNode;
  readonly title: ReactNode;
  readonly sub: ReactNode;
  /** the end: a dot (new, running), an age */
  readonly end?: ReactNode;
  readonly label?: string;
  readonly tip?: string;
  readonly onClick: MouseEventHandler<HTMLButtonElement>;
  readonly menu: EntityMenu;
  /** its pin (pinbutton.tsx): the hover pin in its end's place, the pin mark at rest when pinned */
  readonly pin?: PinOf;
  readonly testId: string;
  /** the row's box (an artifact's row: ⌘⏎ under the pointer, artifacts/front.ts) */
  readonly ref?: Ref<HTMLDivElement>;
}

/** Whether a line box cuts its words: what it holds is wider than it shows. Pure. */
export function isCut(box: { readonly scrollWidth: number; readonly clientWidth: number }): boolean {
  return box.scrollWidth > box.clientWidth + 0.5;
}

/**
 * Whether the line in `el` cuts its words: the line itself, or a box inside it that takes the
 * ellipsis (an artifact's chip title, .b-artchip-title: ui-review m_47044). An inline span measures
 * 0 and never counts.
 */
export function cutIn(el: HTMLElement): boolean {
  return isCut(el) || [...el.querySelectorAll<HTMLElement>('*')].some(isCut);
}

/** Whether the line in `ref` cuts its words, followed as its box changes (a ResizeObserver; none: never cut). */
export function useCut(ref: RefObject<HTMLElement | null>): boolean {
  const [cut, setCut] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (el === null || typeof ResizeObserver === 'undefined') return undefined;
    const ro = new ResizeObserver(() => {
      setCut(cutIn(el));
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
    };
  }, [ref]);
  return cut;
}

/**
 * A card's row: one button (its click, its focus ring, its VoiceOver name), the entity's menu on a
 * right click. Its tip only while its title is cut, the whole title then (designer m_46988: a whole
 * title gets none, never a tip over the row above), left of the card it is in, centred on the row
 * (designer m_47069: above, it covered the section's head; the card and the float are data-tip-box).
 */
export function SideRow({ pic, title, sub, end, label, tip, onClick, menu, pin, testId, ref }: SideRowProps) {
  const l1 = useRef<HTMLSpanElement>(null);
  const cut = useCut(l1);
  // pinned: the quiet pin leads its end at rest; on hover the pin button takes the end's place (pinbutton.tsx)
  const mark = pin?.on === true ? <PinMark testId={`${testId}.pinned`} /> : null;
  return (
    <div ref={ref} className="s3-row" onContextMenu={menu.onContextMenu} data-pinned={pin?.on === true || undefined} data-testid={testId}>
      <Press look="s3-row-body" aria-label={label} {...tipAttrs(cut ? tip : undefined, undefined, 'left')} onClick={onClick} data-row="">
        <span className="s3-row-pic">{pic}</span>
        <span className="s3-row-lines">
          <span ref={l1} className="s3-row-l1">{title}</span>
          <span className="s3-row-l2">{sub}</span>
        </span>
        {/* no room kept for the hover pin: it covers the end's last thing (designer m_49518) */}
        {(end != null || mark !== null) && (
          <span className="s3-row-end">
            {mark}
            {end}
          </span>
        )}
      </Press>
      {pin !== undefined && <PinButton pin={pin} testId={`${testId}.pin`} />}
    </div>
  );
}

/** A card's empty line. */
export function Empty({ children }: { children: ReactNode }) {
  return <p className="s3-empty">{children}</p>;
}
