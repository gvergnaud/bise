// The Task tab's Log (roma idea 3): who did what and why, newest first: its agents' reports and
// their decisions, his answers, its lands, his gestures, agents joining. Asked of the hub when the
// tab shows it and again when the task moves (its last fact); the hub reads it off its loop and
// keeps nothing. Owner: tasks.
import { Fragment, useEffect, useState } from 'react';
import { sentenceCase } from '@/ds/case';
import { useStore, useTaskLog } from '@/store';
import { tasks } from '@/proto/words_tasks.gen';
import { dayWords, timeOf } from '@/artifacts/model';
import { ArtChip, Fold } from '@/ds';
import type { LogLine } from '@/proto';
import { shownName } from '@/agents/model';
import { firstSentence } from './model';
import { openTab } from '@/app/panel/tabs';
import { dayHeads, foldLog, logWords, madeWords, type LogRow } from './log';

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
  // over more than one day: a day head at each day's first row, its rows the hour alone
  const heads = dayHeads(rows.map(({ r }) => r.l.at_ms), (ms) => dayWords(ms, now));
  const days = heads.some((h) => h !== null);
  return (
    <ol className="tk-log" data-testid={testId}>
      {rows.map(({ r, words }, i) => {
        const l = r.l;
        const row = `${testId}.${String(i)}`;
        const head = heads[i] ?? null;
        return (
          <Fragment key={`${String(l.at_ms)}-${String(i)}`}>
          {head !== null && <li className="tk-log-day" data-testid={`${testId}.day.${String(i)}`}>{sentenceCase(head)}</li>}
          <li className="tk-log-row" data-kind={l.kind} data-testid={row}>
            <span className="tk-log-at">{days ? timeOf(l.at_ms) : when(l.at_ms, now)}</span>
            <span className="tk-log-what">
              {l.kind === 'artifact' ? <Made r={r} project={project} testId={row} /> : l.kind === 'report' ? <Report l={l} testId={row} /> : words}
              {l.kind !== 'report' && (l.decisions ?? []).map((d) => (
                <span key={d} className="tk-log-dec">{tasks.log_decision({ text: d })}</span>
              ))}
              {/* his answer folded under its question: one entry (tasks-ux m_65504) */}
              {l.answer != null && <span className="tk-log-dec" data-testid={`${row}.answer`}>{tasks.log_answer({ text: l.answer })}</span>}
            </span>
          </li>
          </Fragment>
        );
      })}
    </ol>
  );
}

/** A report: its first sentence in ink; the rest and its decisions under the ds Fold, closed
 *  (designer m_65808: one line a report, never its whole body; tasks-ui-qa m_65799 #14). */
function Report({ l, testId }: { l: LogLine; testId: string }) {
  const [open, setOpen] = useState(false);
  const text = (l.text ?? '').replace(/^\[report: [a-z]+\]\s*/u, '').trim();
  const head = firstSentence(text);
  const rest = text.slice(text.indexOf(head) + head.length).trim();
  const decisions = l.decisions ?? [];
  const more = rest !== '' || decisions.length > 0;
  return (
    <>
      <span>{tasks.log_report({ who: shownName(l.who), kind: l.report ?? '', text: head })}</span>
      {more && (
        <Fold open={open} onToggle={() => {
          setOpen(!open);
        }} testId={`${testId}.more`}>{tasks.log_more}</Fold>
      )}
      {open && rest !== '' && <span className="tk-log-dec tk-log-rest">{rest}</span>}
      {open && decisions.map((d) => (
        <span key={d} className="tk-log-dec">{tasks.log_decision({ text: d })}</span>
      ))}
    </>
  );
}

/** A version made under the task: who, the artifact as its pink chip (opens it), its version or
 *  its run of versions ('v2→v5'). */
function Made({ r, project, testId }: { r: LogRow; project: string; testId: string }) {
  const m = madeWords(r);
  const art = r.l.art ?? '';
  return (
    <span className="tk-log-made">
      {/* a verb on every row: 'perf made', 'perf updated' (designer m_65569) */}
      <span>{m.verb}</span>
      {/* an artifact is a pink chip, everywhere (taste 13) */}
      <ArtChip title={r.l.text ?? art} glyph={r.l.page === true ? '▤' : '↗'} meta={m.v === '' ? undefined : m.v} keep={24} onClick={() => {
        openTab({ kind: 'artifact', project, id: art });
      }} testId={`${testId}.art`} />
    </span>
  );
}
