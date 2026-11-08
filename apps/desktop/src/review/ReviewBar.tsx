// The review column's sticky bar (page review-guided, pick B): where he is ('01 / 03 · Sidebar ▾',
// a click or ⌘\ opens the outline over the page, never a rail), ‹ › between parts, 'n of 9
// viewed', Guide | By kind, and one slot at its right end for every state of the guide (writing,
// failed, small, his click asked, ready while he read). Its outline: the sections and their
// files, a click jumps there. Pure renders of their props: DiffFiles owns the state and the keys.
import { useRef, type ReactNode } from 'react';
import { Button, Filters, IconButton, PickerPop, Press, tipAttrs } from '@/ds';
import { pages } from '@/proto/words_pages.gen';
import type { GuideEv } from '@/store';
import { aboutTokens, type Section } from './column';

export type Mode = 'guide' | 'kind';

export interface ReviewBarProps {
  /** '01 / 04 · Code' */
  readonly place: string;
  readonly onOutline: () => void;
  /** the outline, open under the place (over the page, never a rail) */
  readonly outline: OutlineProps | null;
  readonly onPrev: () => void;
  readonly onNext: () => void;
  readonly viewed: string;
  readonly mode: Mode;
  /** a guide is here to show (else the Guide segment is off) */
  readonly canGuide: boolean;
  readonly onMode: (m: Mode) => void;
  readonly slot: ReactNode;
  readonly testId: string;
}

export function ReviewBar({ place, onOutline, outline, onPrev, onNext, viewed, mode, canGuide, onMode, slot, testId }: ReviewBarProps) {
  const modes = [
    { id: 'guide', label: pages.review_mode_guide },
    { id: 'kind', label: pages.review_mode_kind },
  ];
  const anchor = useRef<HTMLButtonElement>(null);
  return (
    <div className="rv-bar" data-testid={`${testId}.bar`}>
      <Press look="rv-place" ref={anchor} onClick={onOutline} {...tipAttrs(pages.review_outline_tip)} data-testid={`${testId}.bar.place`}>
        {place} <span className="rv-caret" aria-hidden="true">▾</span>
      </Press>
      {outline !== null && <Outline {...outline} anchor={anchor} />}
      <IconButton label={pages.review_part_prev} size="row" onClick={onPrev} testId={`${testId}.bar.prev`}>‹</IconButton>
      <IconButton label={pages.review_part_next} size="row" onClick={onNext} testId={`${testId}.bar.next`}>›</IconButton>
      <span className="rv-viewed" data-testid={`${testId}.bar.viewed`}>{viewed}</span>
      <span className="rv-bar-gap" />
      {slot}
      <Filters items={canGuide ? modes : modes.slice(1)} on={mode} onPick={(id) => { onMode(id === 'guide' ? 'guide' : 'kind'); }} testId={`${testId}.bar.mode`} />
    </div>
  );
}

export interface GuideSlotProps {
  readonly ev: GuideEv | null;
  /** the guide is drawn now */
  readonly shown: boolean;
  readonly onRun: () => void;
  readonly onShow: () => void;
  readonly testId: string;
}

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
          {pages.review_guide_failed} · <Button kind="ghost" size="sm" onClick={onRun} testId={`${id}.retry`}>{pages.review_retry}</Button>
        </span>
      );
    case 'needs_click':
      return (
        <Button kind="ghost" size="sm" onClick={onRun} title={pages.review_guide_write_tip} testId={`${id}.write`}>
          {pages.review_guide_write({ tokens: aboutTokens(ev.tokens ?? 0), model: ev.model ?? '' })}
        </Button>
      );
    case 'ready':
      return shown ? null : (
        <span className="rv-slot rv-ready" data-testid={id}>
          {pages.review_guide_ready} · <Button kind="ghost" size="sm" onClick={onShow} testId={`${id}.show`}>{pages.review_guide_show}</Button>
        </span>
      );
    default:
      return null;
  }
}

export interface OutlineProps {
  readonly sections: readonly Section[];
  readonly number: (key: string) => string | null;
  readonly onSection: (key: string) => void;
  readonly onClose: () => void;
  readonly testId: string;
}

/** The outline over the page, from the bar's place: each section (its number, its name, its facts), a pick goes there. */
function Outline({ sections, number, onSection, onClose, testId, anchor }: OutlineProps & { anchor: React.RefObject<HTMLButtonElement | null> }) {
  const rows = sections.map((s) => ({ id: s.key, name: [number(s.key), s.title ?? s.label].filter(Boolean).join(' · '), secondary: s.facts }));
  return <PickerPop anchor={anchor} align="start" groups={[{ id: 'sections', rows }]} onPick={onSection} onClose={onClose} testId={`${testId}.outline`} />;
}
