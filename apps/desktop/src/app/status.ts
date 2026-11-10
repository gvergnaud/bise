// The window's one status (designer's desktop-notices page v2, m_21067): what bise does by itself
// (core down or restarted, an update ready, /update's build, a reload) says so in ONE pill in the
// title bar's own row, never under the composer, and nothing moves under it. One at a time: the
// worst wins (went_bad > core down > failed > update > reload > done), the others wait behind it. Pure; the
// pill itself is TitleStatus.tsx. Owner: amb-home.
import type { ReactNode } from 'react';

/**
 * How bad a status is: the higher wins the pill. An update ready ties a reload running; the callers'
 * order puts the update first. `back`: the new app didn't start right (went_bad) wins over core down
 * (go-back-banner, architect m_58270, designer m_58271): a packaged app's core is its own, so a core
 * down after an update is the update's fault, 'start bise' starts the same broken core again, and
 * 'go back' is the one act that fixes it.
 */
export const RANK = { done: 1, ready: 2, running: 2, failed: 3, down: 4, back: 5 } as const;

/** One of its actions: in the pill when there is room (test id `<testId>.<id>`), and in its popover (`<testId>.pop.<id>`). */
export type StatusAct = {
  readonly id: string;
  readonly text: string;
  readonly onClick: () => void;
  readonly disabled?: boolean;
  /** a second act only in the open popover, never in the failure's tone ('Copy report', designer m_59191) */
  readonly quiet?: boolean;
  /** its words in the popover, where acts are buttons in sentence case ('Try again'); else `text` */
  readonly cardText?: string;
};

/**
 * One candidate for the pill: its rank, and what the pill and its popover draw. Each candidate keeps
 * its own test ids: the pill is `<testId>` (data-state `state`), its words `<testId>.text`, the
 * breath `<testId>.breath`, the age `<testId>.age`, the ✕ `<testId>.close`.
 */
export type StatusPart = {
  readonly rank: number;
  readonly testId: string;
  readonly state?: string;
  /** running: the breath first; done: '✓ …' then it folds; err: --c-err 8%, its words in --c-err */
  readonly tone: 'running' | 'done' | 'quiet' | 'err';
  /** the whole words (shas in mono): the pill cuts them with …, the popover shows them whole */
  readonly words: ReactNode;
  /** under 160 px of room: the sign and short words ('reloading…', '▲ couldn't reload') */
  readonly short: string;
  /** running: how long it has run ('12 s'), text-3 at the pill's end */
  readonly age?: string;
  /** done, past its 3 s: the pill fades (220 ms) */
  readonly fold?: boolean;
  readonly acts?: readonly StatusAct[];
  /** ✕: a failure he closes */
  readonly close?: () => void;
  /** dim lines only the popover shows (the core's last log line, the build's last lines), with their own test ids */
  readonly more?: ReactNode;
  /**
   * an offer he can ignore (a newer bise, designer m_45423): the pill stays neutral even when it
   * failed (only its ▲ in --c-err), its words text-2, ' · ' text-3, its action in ink at 500
   */
  readonly calm?: boolean;
};

/** The line to show: the highest rank, the first of equals (the callers' order). Pure. */
export function worst(parts: readonly (StatusPart | null)[]): StatusPart | null {
  let best: StatusPart | null = null;
  for (const p of parts) if (p !== null && (best === null || p.rank > best.rank)) best = p;
  return best;
}
