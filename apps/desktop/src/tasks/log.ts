// A task Log line in words (roma idea 3): the same words as `sb task show --log` (switchboard's
// task_log::words, words::tasks). The line is facts only (bise_proto rows::LogLine); a time is
// formatted at render. Pure. Owner: tasks.
import type { LogLine } from '@/proto';
import { tasks } from '@/proto/words_tasks.gen';

/** a land's tip, short */
const short = (sha: string): string => sha.slice(0, 9);

/** His gesture's words. */
function gestureWords(l: LogLine): string {
  const t = l.text ?? '';
  const what = t === '' ? tasks.log_a_card : t;
  switch (l.gesture) {
    case 'pause':
      return tasks.log_paused;
    case 'resume':
      return tasks.log_resumed;
    case 'done':
      return tasks.log_done;
    case 'reopen':
      return tasks.log_reopened;
    case 'rename':
      return tasks.log_renamed({ name: t });
    case 'merge':
      return tasks.log_merged({ into: t });
    case 'why':
      return tasks.log_why({ text: t });
    case 'goal':
      return tasks.log_goal({ text: t });
    case 'tick':
      return tasks.log_ticked({ n: t });
    case 'untick':
      return tasks.log_unticked({ n: t });
    case 'ongoing':
      return tasks.log_ongoing;
    case 'move_in':
      return tasks.log_moved_in({ what });
    case 'move_out':
      return tasks.log_moved_out({ what });
    // main's Restart is said as main's, never his (architect m_65555)
    case 'restart':
      return l.who === 'user' ? tasks.log_restarted : tasks.log_restarted_by({ who: l.who });
    default:
      return '';
  }
}

/** A Log row: its line, and for back-to-back versions of one artifact by one agent the oldest
 *  version of the run (`from`; the line is the newest), folded into one row ('v2→v5'). */
export interface LogRow {
  readonly l: LogLine;
  readonly from: number;
}

/** The lines as rows, back-to-back versions of one artifact folded (the same rule as
 *  `sb task show --log`, task_log::fold). Pure. */
export function foldLog(lines: readonly LogLine[]): LogRow[] {
  const out: LogRow[] = [];
  for (const l of lines) {
    const prev = out.at(-1);
    if (prev !== undefined && l.kind === 'artifact' && prev.l.kind === 'artifact' && prev.l.art === l.art && (prev.l.page ?? false) === (l.page ?? false) && prev.l.who === l.who) {
      out[out.length - 1] = { l: prev.l, from: l.v ?? 0 };
      continue;
    }
    out.push({ l, from: l.v ?? 0 });
  }
  return out;
}

/** An artifact row's words around its chip: who made it, and the version or the run. */
export function madeWords(r: LogRow): { readonly who: string; readonly v: string } {
  const v = r.l.v ?? 0;
  return { who: r.l.who, v: r.from < v ? `v${String(r.from)}→v${String(v)}` : `v${String(v)}` };
}

/** A line's words (its decisions apart: the row lists them under it); '' a kind this window
 *  doesn't know (the row is left out). Pure. */
export function logWords(l: LogLine): string {
  switch (l.kind) {
    case 'report':
      return tasks.log_report({ who: l.who, kind: l.report ?? '', text: l.text ?? '' });
    case 'answer':
      return tasks.log_answer({ text: l.text ?? '' });
    case 'land':
      return tasks.log_land({ who: l.who, n: l.commits ?? 0, target: l.target ?? '', sha: short(l.sha ?? '') });
    case 'join':
      return tasks.log_join({ who: l.who });
    case 'gesture':
      return gestureWords(l);
    case 'question':
      return tasks.log_asked({ who: l.who, text: l.text ?? '' });
    case 'artifact':
      return tasks.log_made({ who: l.who, title: l.text ?? '', v: l.v ?? 0 });
    case 'break':
      return l.life === 'failed' ? tasks.log_crashed({ who: l.who, why: l.text ?? '' }) : tasks.log_stopped({ who: l.who, why: l.text ?? '' });
    default:
      return '';
  }
}
