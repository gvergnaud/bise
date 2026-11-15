// The Task tab's Log (roma idea 3): who did what and why, newest first: its agents' reports and
// their decisions, his answers, its lands, his gestures, agents joining. Asked of the hub when the
// tab shows it and again when the task moves (its last fact); the hub reads it off its loop and
// keeps nothing. Owner: tasks.
import { useEffect } from 'react';
import { useStore, useTaskLog } from '@/store';
import { tasks } from '@/proto/words_tasks.gen';
import { dayWords, timeOf } from '@/artifacts/model';
import { logWords } from './log';

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
  const lines = log.lines.flatMap((l) => {
    const words = logWords(l);
    return words === '' ? [] : [{ l, words }];
  });
  if (lines.length === 0) return <p className="tk-meta" data-testid={`${testId}.none`}>{tasks.log_none}</p>;
  return (
    <ol className="tk-log" data-testid={testId}>
      {lines.map(({ l, words }, i) => (
        <li key={`${String(l.at_ms)}-${String(i)}`} className="tk-log-row" data-kind={l.kind} data-testid={`${testId}.${String(i)}`}>
          <span className="tk-log-at">{when(l.at_ms, now)}</span>
          <span className="tk-log-what">
            {words}
            {(l.decisions ?? []).map((d) => (
              <span key={d} className="tk-log-dec">{tasks.log_decision({ text: d })}</span>
            ))}
          </span>
        </li>
      ))}
    </ol>
  );
}
