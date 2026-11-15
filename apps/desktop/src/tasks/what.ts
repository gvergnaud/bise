// A task's `<status> · <what>` line, never blank (tasks-ux m_64508, m_64563): the right bar, the
// Board and the Task tab read it. The pick rule is the hub's (bise_proto rows::WorkRow::what), the
// words the catalog's (words::tasks), both sides pinned by rust/proto/fixtures/work_what.jsonl
// (architect m_64809). Times are formatted here at render from the row's absolute `at_ms` (`past`,
// `next`): a time sent as text would go stale. Pure. Owner: tasks.
import type { WorkRow } from '@/proto';
import { tasks } from '@/proto/words_tasks.gen';

const DAY_MS = 24 * 3_600_000;

/** `today`, `yesterday`, `3 days ago`: whole days since `at` */
function daysAgo(at: number, now: number): string {
  const n = Math.floor(Math.max(0, now - at) / DAY_MS);
  if (n === 0) return tasks.today;
  if (n === 1) return tasks.yesterday;
  return tasks.days_ago({ n });
}

/** the row's line: `past` formats a past time, `next` one to come */
export function whatOf(r: WorkRow, now: number, past: (ms: number) => string, next: (ms: number) => string): string {
  const at = r.at_ms ?? undefined;
  const p = (): string => (at === undefined ? '' : past(at));
  const n = (): string => (at === undefined ? '' : next(at));
  const who = r.who ?? '';
  const about = r.about ?? '';
  const needs = r.needs ?? 0;
  switch (r.status) {
    case 'needs_you':
      // a card of it on a page: that page (the hub joins it; tasks-ux: Waiting is never blank)
      if (r.page != null) return tasks.needs_page({ page: r.page });
      return needs > 0 ? tasks.needs_questions({ n: needs }) : tasks.status_needs_you;
    case 'working':
      return who !== '' ? tasks.working_who({ who }) : tasks.status_working;
    case 'waiting':
      if (about === 'reply') return tasks.waiting_reply({ who });
      if (about === 'timer') return tasks.waiting_timer({ at: n() });
      if (about === 'watch') return tasks.waiting_watch({ who });
      // no agent yet: say so, never a bare Waiting (tasks-ux m_65131)
      if ((r.agents ?? []).length === 0) return tasks.waiting_no_agent;
      return at !== undefined ? tasks.waiting_quiet({ at: p() }) : tasks.status_waiting;
    case 'stalled':
      if (about === 'crash') return tasks.stalled_crash({ who, at: p() });
      return at !== undefined ? tasks.stalled_quiet({ at: p() }) : tasks.status_stalled;
    case 'to_review':
      // its agents' open PR (the hub joins it from the places)
      return r.pr != null ? tasks.review_pr({ n: r.pr }) : tasks.status_to_review;
    case 'paused':
      return about === 'until' && at !== undefined ? tasks.paused_until({ at: n() }) : tasks.paused_by_you;
    case 'done':
      return r.done ? tasks.done_when({ when: daysAgo(r.done.at_ms, now) }) : tasks.status_done;
    case 'ongoing':
      return tasks.status_ongoing;
    default:
      return tasks.status_unknown;
  }
}
