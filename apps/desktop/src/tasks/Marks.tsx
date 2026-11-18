// The tasks' shared pieces (brief "Shared pieces", designer's look rules): the status mark (TaskMark:
// one drawn SVG set in a 12 px box, each with its tip: the task's line), the neutral task chip (the mark,
// then the name cut at a word, never pink), the chips (the main one in clear, '+N' whose tip lists
// the others), and the openers of the Task tab and the Board. Tokens only (tasks.css). Owner: tasks.
import type { ReactNode } from 'react';
import { Press, tipAttrs, WordCut } from '@/ds';
import { openTab } from '@/app/panel/tabs';
import type { WorkRow, WorkStatus } from '@/proto';
import { tasks } from '@/proto/words_tasks.gen';
import { age } from '@/inbox/InboxCard';
import './tasks.css';

/** The paused line: 'paused by you · just now' in its first minute, then '· 4 h ago' ('ago' only
 *  after a real duration, tasks-ux m_65628). Pure. */
export function pausedWords(ms: number): string {
  return ms < 60_000 ? tasks.paused_just_now : tasks.paused_since({ ago: age(ms) });
}

/** 'new since you looked' (roma idea 2, designer): our 6 px ink dot after the name, never blue,
 *  never pink (pink is what waits on him); its words for a reader in its label. */
export function NewDot({ on, testId }: { on: boolean; testId?: string }) {
  if (!on) return null;
  return <span className="tk-new" role="img" aria-label={tasks.new_since_looked} {...tipAttrs(tasks.new_since_looked)} {...(testId === undefined ? {} : { 'data-testid': testId })} />;
}

/** a status's word alone (the mark's aria label, a column's strip) */
export function statusWord(s: WorkStatus): string {
  switch (s) {
    case 'needs_you':
      return tasks.status_needs_you;
    case 'working':
      return tasks.status_working;
    case 'waiting':
      return tasks.status_waiting;
    case 'stalled':
      return tasks.status_stalled;
    case 'to_review':
      return tasks.status_to_review;
    case 'paused':
      return tasks.col_paused.toLowerCase();
    case 'done':
      return tasks.status_done;
    case 'ongoing':
      return tasks.status_ongoing;
    default:
      return tasks.status_unknown;
  }
}

/** Each status's drawing in a 12 px box (tasks-shine batch 2, designer m_66582, the shapes signed in
 *  6b): a dot (needs you, working), a solid ring (waiting, unknown), a ring with its center dot (to
 *  review), ✗ (stalled), ‖ (paused), ✓ (done), ↻ (ongoing). Its color is its css (data-st). */
const DOT = <circle cx="6" cy="6" r="3.25" fill="currentColor" stroke="none" />;
const RING = <circle cx="6" cy="6" r="3.5" fill="none" />;
const SHAPE: Record<WorkStatus, ReactNode> = {
  needs_you: DOT,
  working: DOT,
  waiting: RING,
  to_review: (
    <>
      {RING}
      <circle cx="6" cy="6" r="1.25" fill="currentColor" stroke="none" />
    </>
  ),
  stalled: <path d="M3.75 3.75l4.5 4.5M8.25 3.75l-4.5 4.5" />,
  paused: <path d="M4.5 3.25v5.5M7.5 3.25v5.5" />,
  done: <path d="M3.25 6.25l1.9 1.9 3.6-4.1" />,
  ongoing: (
    <>
      <path d="M8.6 4.4A3.4 3.4 0 1 0 9.4 6.6" />
      <path d="M8.9 2.6v2h-2" />
    </>
  ),
  unknown: RING,
};

/** The task's status mark, one drawn set everywhere (the Board, the right bar, the Task tab, the
 *  chips, ⌘K, the pickers): `tip` its tip (the task's line), none for a mark inside a control that
 *  has its own. */
export function TaskMark({ status, tip, testId }: { status: WorkStatus; tip?: string; testId?: string }) {
  return (
    <span className="tk-mark" data-st={status} role="img" aria-label={statusWord(status)} data-testid={testId} {...(tip === undefined ? {} : tipAttrs(tip))}>
      <svg viewBox="0 0 12 12" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
        {SHAPE[status]}
      </svg>
    </span>
  );
}

/** The todo list's box (his m_66588, designer m_66969): in a row's mark slot, a 12 px box in a 24 px
 *  hit area, its stroke the status's color; checked: ✓ inside, text-3; `ready`: open with every
 *  done-when item ticked, an ink box ready to check. Its click is his Mark done (open) or Reopen
 *  (checked); the rest of the row still opens the Task tab. */
export function TaskCheck({ status, checked, ready, onToggle, testId }: { status: WorkStatus; checked: boolean; ready: boolean; onToggle: () => void; testId?: string }) {
  const word = checked ? tasks.act_reopen : tasks.act_done;
  return (
    <Press look="tk-box" role="checkbox" aria-checked={checked} aria-label={word} data-st={status} data-checked={checked || undefined} data-ready={ready || undefined}
      {...tipAttrs(word)} onClick={(e) => {
        e.stopPropagation();
        onToggle();
      }} data-testid={testId}>
      <svg viewBox="0 0 12 12" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round">
        <rect x="1.5" y="1.5" width="9" height="9" rx="2.5" />
        {checked && <path d="M3.6 6.2l1.6 1.6 3.2-3.5" />}
      </svg>
    </Press>
  );
}

/** Opens a task's tab in the panel (over the Board when it is in front: the Board stays one click back). */
export function openTask(project: string, id: string): void {
  openTab({ kind: 'task', project, id });
}

/** Opens a project's Board in the panel ('': every project's). */
export function openBoard(project: string): void {
  openTab({ kind: 'tasks', project });
}

/** The task chip, one look everywhere (tasks-shine batch 1, designer m_66523): neutral, 20 high,
 *  its mark in an 8 px box, then its name cut at a word to its room (14ch at most, the ds WordCut);
 *  its tip the whole name and its line. A narrow place shrinks it to its mark alone (its css). */
export function TaskChip({ project, w, line, testId }: { project: string; w: Pick<WorkRow, 'id' | 'name' | 'status'>; line: string; testId?: string }) {
  return (
    <Press look="tk-chip" data-testid={testId} onClick={(e) => {
      e.stopPropagation();
      openTask(project, w.id);
    }} {...tipAttrs(tasks.chip_tip({ name: w.name, what: line }))}>
      <span className="tk-chip-mark"><TaskMark status={w.status} /></span>
      <WordCut className="tk-chip-name" text={w.name} />
    </Press>
  );
}

/** Many tasks: the first in clear, then '+N' in text-3 whose tip lists the others (its click opens the next). */
export function TaskChips({ project, list, lineOf, testId }: { project: string; list: readonly Pick<WorkRow, 'id' | 'name' | 'status'>[]; lineOf: (id: string) => string; testId?: string }) {
  const [first, ...rest] = list;
  if (first === undefined) return null;
  return (
    <span className="tk-chips">
      <TaskChip project={project} w={first} line={lineOf(first.id)} {...(testId === undefined ? {} : { testId })} />
      {rest.length > 0 && (
        <Press look="tk-more" data-testid={testId === undefined ? undefined : `${testId}.more`} onClick={(e) => {
          e.stopPropagation();
          const next = rest[0];
          if (next !== undefined) openTask(project, next.id);
        }} {...tipAttrs(rest.map((x) => `${x.name} · ${lineOf(x.id)}`).join('\n'))}>
          {tasks.chips_more({ n: rest.length })}
        </Press>
      )}
    </span>
  );
}
