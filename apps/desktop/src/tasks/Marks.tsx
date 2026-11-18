// The tasks' shared pieces (brief "Shared pieces", designer's look rules): the status mark (a 6 px
// dot or ring, or its glyph, each with its tip: the task's line), the neutral task chip (the mark,
// then the name cut at a word, never pink), the chips (the main one in clear, '+N' whose tip lists
// the others), and the openers of the Task tab and the Board. Tokens only (tasks.css). Owner: tasks.
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

/** the glyph statuses: Stalled ✗ (err), Paused ‖, Done ✓, Ongoing ↻; the others are a dot or a ring */
const GLYPH: Partial<Record<WorkStatus, string>> = { stalled: '✗', paused: '‖', done: '✓', ongoing: '↻' };

/** The status mark: `tip` its tip (the task's line), none for a mark inside a control that has its own. */
export function StatusMark({ status, tip, testId }: { status: WorkStatus; tip?: string; testId?: string }) {
  return (
    <span className="tk-mark" data-st={status} role="img" aria-label={statusWord(status)} data-testid={testId} {...(tip === undefined ? {} : tipAttrs(tip))}>
      {GLYPH[status] ?? null}
    </span>
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
      <span className="tk-chip-mark"><StatusMark status={w.status} /></span>
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
