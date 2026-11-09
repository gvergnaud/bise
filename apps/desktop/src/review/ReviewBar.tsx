// The review column's sticky bar (page review-guided, pick B): where he is ('01 / 03 · Give the row's
// end one owner ▾', a click or ⌘\ opens the outline over the page, never a rail), ‹ › between parts,
// 'n of 9 viewed', one slot for every state of the guide (writing, failed, his click asked, ready
// while he read), and Guide | By kind, always both (Guide off until a guide is there; only a small
// change, which never gets one, drops it). A hairline at its foot while code scrolls under it. In a
// narrow column the place says the part's area, the count '0/9', the button its short words: the
// button never gets cut. The bar never leaves its column (designer m_57825, m_57833): when it would,
// things give way in turn (FIT): the slot's model words (into its tip), then its token count, then
// the viewed count, last the part's name (whole or …; one word is never cut, only dropped whole);
// Guide | By kind always fits. Renders of their props but that fit: DiffFiles owns the state and keys.
import { useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from 'react';
import { Button, Filters, IconButton, PickerPop, Press, tipAttrs } from '@/ds';
import { pages } from '@/proto/words_pages.gen';
import type { GuideEv } from '@/store';
import { aboutTokens, type Section } from './column';

export type Mode = 'guide' | 'kind';

export interface ReviewBarProps {
  /** '01 / 03 · Give the row's end one owner' */
  readonly place: string;
  /** 'Sidebar': a narrow column's (its area alone, never cut for its number) */
  readonly placeShort: string;
  /** one section only: nothing to walk (no ‹ ›, no outline) */
  readonly single: boolean;
  readonly onOutline: () => void;
  /** the outline, open under the place (over the page, never a rail) */
  readonly outline: OutlineProps | null;
  /** null at the first (last) section: its button is off */
  readonly onPrev: (() => void) | null;
  readonly onNext: (() => void) | null;
  readonly viewed: { readonly n: number; readonly total: number };
  readonly mode: Mode;
  /** the Guide segment: pickable (a guide is here), off with its tip's reason, or gone (small change) */
  readonly guide: { readonly on: true } | { readonly on: false; readonly why: string } | null;
  readonly onMode: (m: Mode) => void;
  readonly slot: ReactNode;
  /** code scrolls under it: its hairline */
  readonly under: boolean;
  readonly testId: string;
}

export function ReviewBar({ place, placeShort, single, onOutline, outline, onPrev, onNext, viewed, mode, guide, onMode, slot, under, testId }: ReviewBarProps) {
  const anchor = useRef<HTMLButtonElement>(null);
  const bar = useRef<HTMLDivElement>(null);
  const fit = useFit(bar);
  const modes = [
    ...(guide === null ? [] : [{ id: 'guide', label: pages.review_mode_guide, ...(guide.on ? {} : { disabled: true, tip: guide.why }) }]),
    { id: 'kind', label: pages.review_mode_kind },
  ];
  return (
    <div ref={bar} className="rv-bar" data-under={under || undefined} data-fit={FIT[fit]} data-testid={`${testId}.bar`}>
      {single ? (
        <span className="rv-place" data-testid={`${testId}.bar.place`}>
          <span className="rv-wide">{place}</span>
          <span className="rv-narrow">{placeShort}</span>
        </span>
      ) : (
        <>
          <Press look="rv-place" ref={anchor} onClick={onOutline} {...tipAttrs(pages.review_outline_tip)} data-testid={`${testId}.bar.place`}>
            <span className="rv-wide">{place}</span>
            <span className="rv-narrow">{placeShort}</span>
            <span className="rv-caret" aria-hidden="true">▾</span>
          </Press>
          {outline !== null && <Outline {...outline} anchor={anchor} />}
          <span className="rv-steps">
            <IconButton label={pages.review_part_prev} size="row" disabled={onPrev === null} onClick={onPrev ?? undefined} testId={`${testId}.bar.prev`}>‹</IconButton>
            <IconButton label={pages.review_part_next} size="row" disabled={onNext === null} onClick={onNext ?? undefined} testId={`${testId}.bar.next`}>›</IconButton>
          </span>
        </>
      )}
      <span className="rv-viewed" data-testid={`${testId}.bar.viewed`}>
        <span className="rv-wide">{pages.review_viewed_count(viewed)}</span>
        <span className="rv-narrow">{pages.review_viewed_short(viewed)}</span>
      </span>
      <span className="rv-bar-gap" />
      {slot}
      <Filters items={modes} on={mode} onPick={(id) => { onMode(id === 'guide' ? 'guide' : 'kind'); }} testId={`${testId}.bar.mode`} />
    </div>
  );
}

/**
 * What the bar gave way, step by step (review.css .rv-bar[data-fit~=...]): none; the slot's short
 * words (no model: its tip has it); its bare words (no token count); the count gone; the part's
 * name cut with …; the part's name gone.
 */
const FIT = [undefined, 'slot-short', 'slot-bare', 'slot-bare count-off', 'slot-bare count-off place-cut', 'slot-bare count-off place-off'] as const;

/** The words a part's name shows now (the wide or the narrow one: the one displayed). */
function shownPlace(place: Element): string {
  const parts = [...place.querySelectorAll('.rv-wide, .rv-narrow')].filter((p) => getComputedStyle(p).display !== 'none');
  return (parts[0] ?? place).textContent.trim();
}

/**
 * The bar's fit (FIT's steps): after each render, before paint, one step more while its row is
 * wider than the bar, or while a one-word part name is cut (it is dropped whole instead); a new
 * width starts again from none.
 */
function useFit(bar: RefObject<HTMLDivElement | null>): number {
  const [fit, setFit] = useState(0);
  // eslint-disable-next-line react-hooks/exhaustive-deps -- after every render: the slot's words change (writing, then 'Write a guide · …')
  useLayoutEffect(() => {
    const el = bar.current;
    if (el === null || fit >= FIT.length - 1) return;
    const place = el.querySelector('.rv-place');
    const oneWordCut = place !== null && place.scrollWidth > place.clientWidth && !/\s/u.test(shownPlace(place));
    if (el.scrollWidth <= el.clientWidth && !oneWordCut) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- the row's width: known only after the commit, before paint
    setFit(fit + 1);
  });
  useLayoutEffect(() => {
    const el = bar.current;
    if (el === null || typeof ResizeObserver === 'undefined') return undefined;
    let width = el.clientWidth;
    const seen = new ResizeObserver(() => {
      if (el.clientWidth === width) return;
      width = el.clientWidth;
      setFit(0);
    });
    seen.observe(el);
    return () => {
      seen.disconnect();
    };
  }, [bar]);
  return fit;
}

export interface GuideSlotProps {
  readonly ev: GuideEv | null;
  /** the guide is drawn now */
  readonly shown: boolean;
  readonly onRun: () => void;
  readonly onShow: () => void;
  readonly testId: string;
}

/** A model's name without its provider ('claude-opus-5-5'); its whole id goes in the tip. */
const modelName = (id: string) => id.slice(id.lastIndexOf('/') + 1);

/** The bar's one slot for the guide's state; nothing when there's nothing to say. */
export function GuideSlot({ ev, shown, onRun, onShow, testId }: GuideSlotProps) {
  const id = `${testId}.bar.guide`;
  switch (ev?.state) {
    case 'writing':
      return <span className="rv-slot" data-testid={id}>{pages.review_guide_writing}</span>;
    case 'small':
      return <span className="rv-slot" data-testid={id}>{pages.review_guide_small}</span>;
    case 'failed':
      return (
        <span className="rv-slot" data-testid={id}>
          {pages.review_guide_failed} · <Button size="sm" onClick={onRun} testId={`${id}.retry`}>{pages.review_retry}</Button>
        </span>
      );
    case 'needs_click': {
      const tokens = aboutTokens(ev.tokens ?? 0);
      const model = modelName(ev.model ?? '');
      return (
        <span className="rv-slot" data-testid={id}>
          <Button size="sm" onClick={onRun} title={pages.review_guide_write_tip({ model: ev.model ?? '' })} testId={`${id}.write`}>
            <span className="rv-wide">{pages.review_guide_write({ tokens, model })}</span>
            <span className="rv-narrow">{pages.review_guide_write_short({ tokens })}</span>
            <span className="rv-bare">{pages.review_guide_write_bare}</span>
          </Button>
        </span>
      );
    }
    case 'ready':
      return shown ? null : (
        <span className="rv-slot rv-ready" data-testid={id}>
          {pages.review_guide_ready} · <Button size="sm" onClick={onShow} testId={`${id}.show`}>{pages.review_guide_show}</Button>
        </span>
      );
    default:
      return null;
  }
}

export interface OutlineProps {
  readonly sections: readonly Section[];
  readonly number: (key: string) => string | null;
  /** the section he is in: the picker's selection */
  readonly current: string;
  /** a section's files he marked viewed */
  readonly viewed: (key: string) => number;
  readonly onSection: (key: string) => void;
  readonly onClose: () => void;
  readonly testId: string;
}

/**
 * The outline over the page, from the bar's place: each section, its name on the first line (a
 * part's title, a critical one's ink dot before it, as in the column), its area, facts and viewed
 * files on the second; the one he is in selected. A pick goes there.
 */
function Outline({ sections, number, current, viewed, onSection, onClose, testId, anchor }: OutlineProps & { anchor: RefObject<HTMLButtonElement | null> }) {
  const rows = sections.map((s) => {
    const n = viewed(s.key);
    return {
      id: s.key,
      name: [number(s.key), s.title ?? s.label].filter(Boolean).join(' · '),
      secondary: [s.title === null ? '' : s.label, s.facts, n > 0 ? pages.review_part_viewed({ n }) : ''].filter(Boolean).join(' · '),
      ...(s.weight === 'critical' ? { glyph: <i className="rv-dot" {...tipAttrs(pages.review_part_critical)} /> } : {}),
      current: s.key === current,
    };
  });
  return <PickerPop anchor={anchor} align="start" groups={[{ id: 'sections', rows }]} active={current} onPick={onSection} onClose={onClose} testId={`${testId}.outline`} />;
}
