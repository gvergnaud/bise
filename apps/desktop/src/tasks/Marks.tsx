// The tasks' shared pieces (brief "Shared pieces", designer's look rules): the status mark (a 6 px
// dot or ring, or its glyph, each with its tip: the task's line), the neutral task chip (the mark,
// then the name cut at a word, never pink), the chips (the main one in clear, '+N' whose tip lists
// the others), and the openers of the Task tab and the Board. Tokens only (tasks.css). Owner: tasks.
import { Press, tipAttrs } from '@/ds';
import { openTab } from '@/app/panel/tabs';
import type { WorkRow, WorkStatus } from '@/proto';
import { tasks } from '@/proto/words_tasks.gen';
import { cutAtWord } from './model';
import './tasks.css';

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

/** The task chip: neutral, its mark then its name cut at a word; its tip the whole name and its line. */
export function TaskChip({ project, w, line, max = 28, testId }: { project: string; w: Pick<WorkRow, 'id' | 'name' | 'status'>; line: string; max?: number; testId?: string }) {
  const name = cutAtWord(w.name, max);
  return (
    <Press look="tk-chip" data-testid={testId} onClick={(e) => {
      e.stopPropagation();
      openTask(project, w.id);
    }} {...tipAttrs(tasks.chip_tip({ name: w.name, what: line }))}>
      <StatusMark status={w.status} />
      <span className="tk-chip-name">{name}</span>
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
