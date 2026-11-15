// A task Log line in words (roma idea 3): the same words as `sb task show --log` (switchboard's
// task_log::words, words::tasks). The line is facts only (bise_proto rows::LogLine); a time is
// formatted at render. Pure. Owner: tasks.
import type { LogLine } from '@/proto';
import { tasks } from '@/proto/words_tasks.gen';
import { shownName } from '@/agents/model';

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
    // main's move or Restart is said as bise's ('main' never shows in the app, designer m_65569),
    // never his (architect m_65555, m_65565)
    case 'move_in':
      return l.who === 'user' ? tasks.log_moved_in({ what }) : tasks.log_moved_in_by({ who: shownName(l.who), what });
    case 'move_out':
      return l.who === 'user' ? tasks.log_moved_out({ what }) : tasks.log_moved_out_by({ who: shownName(l.who), what });
    case 'restart':
      return l.who === 'user' ? tasks.log_restarted : tasks.log_restarted_by({ who: shownName(l.who) });
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

/** An artifact row's words around its chip: who and its verb ('perf made', 'perf updated'), and
 *  the version or the run ('' for a first version alone; designer m_65569). Pure. */
export function madeWords(r: LogRow): { readonly verb: string; readonly v: string } {
  const v = r.l.v ?? 0;
  const who = shownName(r.l.who);
  if (r.from < v) return { verb: tasks.log_updated_verb({ who }), v: `v${String(r.from)}→v${String(v)}` };
  return v > 1 ? { verb: tasks.log_updated_verb({ who }), v: `v${String(v)}` } : { verb: tasks.log_made_verb({ who }), v: '' };
}

/** A break in words: a crash says so once ('auth stopped (crash): <why>', designer m_65569). Pure. */
function breakWords(l: LogLine): string {
  const why = (l.text ?? '').trim();
  const crash = why.startsWith('crash:');
  const rest = crash ? why.slice('crash:'.length).trim() : why;
  return crash || l.life === 'failed' ? tasks.log_crashed({ who: l.who, why: rest }) : tasks.log_stopped({ who: l.who, why: rest });
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
      return (l.v ?? 0) > 1 ? tasks.log_updated({ who: shownName(l.who), title: l.text ?? '', v: l.v ?? 0 }) : tasks.log_made({ who: shownName(l.who), title: l.text ?? '' });
    case 'break':
      return breakWords(l);
    default:
      return '';
  }
}
