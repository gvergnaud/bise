// The Task tab's Log (roma idea 3): who did what and why, newest first: its agents' reports and
// their decisions, his answers, its lands, his gestures, agents joining. Asked of the hub when the
// tab shows it and again when the task moves (its last fact); the hub reads it off its loop and
// keeps nothing. Owner: tasks.
import { useEffect } from 'react';
import { useStore, useTaskLog } from '@/store';
import { tasks } from '@/proto/words_tasks.gen';
import { dayWords, timeOf } from '@/artifacts/model';
import { ArtChip } from '@/ds';
import { openTab } from '@/app/panel/tabs';
import { foldLog, logWords, madeWords, type LogRow } from './log';

/** a line's time: the hour today, else its day and hour (24 h, designer m_64958) */
function when(ms: number, now: number): string {
  const day = dayWords(ms, now);
  return day === 'today' ? timeOf(ms) : `${day} ${timeOf(ms)}`;
}

export function TaskLog({ project, id, last, now, testId }: { project: string; id: string; last: number; now: number; testId: string }) {
  const store = useStore();
  const log = useTaskLog(project, id);
  // asked when shown and when the task's newest fact moves (a report, a gesture, a delivery)
  useEffect(() => {
    store.askTaskLog(project, id);
  }, [store, project, id, last]);
  if (log === null) return <p className="tk-meta" data-testid={`${testId}.loading`}>{tasks.log_loading}</p>;
  if (log.error != null) return <p className="tk-meta" data-testid={`${testId}.error`}>{tasks.log_failed({ why: log.error })}</p>;
  const rows = foldLog(log.lines).flatMap((r) => {
    const words = logWords(r.l);
    return words === '' ? [] : [{ r, words }];
  });
  if (rows.length === 0) return <p className="tk-meta" data-testid={`${testId}.none`}>{tasks.log_none}</p>;
  return (
    <ol className="tk-log" data-testid={testId}>
      {rows.map(({ r, words }, i) => {
        const l = r.l;
        const row = `${testId}.${String(i)}`;
        return (
          <li key={`${String(l.at_ms)}-${String(i)}`} className="tk-log-row" data-kind={l.kind} data-testid={row}>
            <span className="tk-log-at">{when(l.at_ms, now)}</span>
            <span className="tk-log-what">
              {l.kind === 'artifact' ? <Made r={r} project={project} testId={row} /> : words}
              {(l.decisions ?? []).map((d) => (
                <span key={d} className="tk-log-dec">{tasks.log_decision({ text: d })}</span>
              ))}
              {/* his answer folded under its question: one entry (tasks-ux m_65504) */}
              {l.answer != null && <span className="tk-log-dec" data-testid={`${row}.answer`}>{tasks.log_answer({ text: l.answer })}</span>}
            </span>
          </li>
        );
      })}
    </ol>
  );
}

/** A version made under the task: who, the artifact as its pink chip (opens it), its version or
 *  its run of versions ('v2→v5'). */
function Made({ r, project, testId }: { r: LogRow; project: string; testId: string }) {
  const m = madeWords(r);
  const art = r.l.art ?? '';
  return (
    <span className="tk-log-made">
      <span>{m.who}</span>
      {/* an artifact is a pink chip, everywhere (taste 13) */}
      <ArtChip title={r.l.text ?? art} glyph={r.l.page === true ? '▤' : '↗'} meta={m.v} keep={24} onClick={() => {
        openTab({ kind: 'artifact', project, id: art });
      }} testId={`${testId}.art`} />
    </span>
  );
}
