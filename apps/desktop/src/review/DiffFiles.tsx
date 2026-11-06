// A diff's files, one ds Diff each (the review's body, and a .diff artifact's in the panel:
// try-diff-open, architect m_52963: one component for both, never a copy). Its keys (tui-parity V6):
// tab / shift+tab next / previous file, j k scroll, esc `onEscape`; V7 / L11: drag or shift+↑↓
// select whole lines, a typed key quotes them in `to`'s thread. Each file's code is colored by its
// language (diff-syntax: syntax.ts), after a first plain paint. Owner: nm-pages-b.
import { useDeferredValue, useEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent, type ReactNode } from 'react';
import { copyText, QuotePop } from '@/quote/QuotePop';
import { flatRows, handQuotes, quotesOf, range, type RowAt, type Sel } from './quote';
import { Button, Diff, Empty, type DiffLine as DsLine } from '@/ds';
import { pages } from '@/proto/words_pages.gen';
import { diffRuns, langOfPath } from './syntax';
import { useRouter } from '@/app/router';
import type { Diff as DiffEv } from '@/store';
import { fileHead, linesOf } from './model';
import './review.css';

export interface DiffFilesProps {
  readonly project: string;
  /** null while it comes */
  readonly diff: DiffEv | null;
  /** the words while it comes ('loading perf's changes…') */
  readonly loading: string;
  /** the hub refused it (useDiffError): its words in place of 'loading…', with `onRetry` */
  readonly error?: string | null;
  readonly onRetry?: () => void;
  /** the empty state's title when nothing differs */
  readonly none: string;
  /** whose thread a quote goes to, and what it quotes ('perf vs main', 'cu-upload at dd86c8a') */
  readonly to: string;
  readonly from: string;
  readonly onEscape: () => void;
  /** its test ids: `<testId>.files`, a file `<testId>.file.<i>`, its line `<testId>.file.<i>.<l>` */
  readonly testId?: string;
}

export function DiffFiles({ project, diff, loading, error, onRetry, none, to, from, onEscape, testId = 'review' }: DiffFilesProps) {
  const router = useRouter();
  const body = useRef<HTMLDivElement>(null);
  const at = useRef(-1);
  useEffect(() => {
    body.current?.focus();
  }, []);
  const fileLines = useMemo(() => diff?.files.map(linesOf) ?? [], [diff]);
  const paths = useMemo(() => diff?.files.map((f) => f.path) ?? [], [diff]);
  const rows = useMemo(() => flatRows(fileLines), [fileLines]);
  const starts = useMemo(() => fileLines.reduce<number[]>((acc, _ls, i) => [...acc, i === 0 ? 0 : (acc[i - 1] ?? 0) + (fileLines[i - 1]?.length ?? 0)], []), [fileLines]);
  const [sel, setSel] = useState<Sel | null>(null);
  const [dragging, setDragging] = useState(false);
  const lineId = useMemo(() => new RegExp(`^${testId.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&')}\\.file\\.(\\d+)\\.(\\d+)$`, 'u'), [testId]);
  const rowOf = (el: EventTarget | null): number | undefined => {
    const id = el instanceof Element ? el.closest('.b-dl')?.getAttribute('data-testid') : null;
    const m = id == null ? null : lineId.exec(id);
    if (m === null) return undefined;
    const s = starts[Number(m[1])];
    return s === undefined ? undefined : s + Number(m[2]);
  };
  const onMouseDown = (e: MouseEvent) => {
    const i = rowOf(e.target);
    if (i === undefined || e.button !== 0) return;
    e.preventDefault();
    body.current?.focus();
    setSel({ anchor: i, head: i });
    setDragging(true);
  };
  const onMouseOver = (e: MouseEvent) => {
    if (!dragging) return;
    const i = rowOf(e.target);
    if (i !== undefined) setSel((s) => (s === null ? s : { ...s, head: i }));
  };
  useEffect(() => {
    if (!dragging) return;
    const up = () => {
      setDragging(false);
    };
    window.addEventListener('mouseup', up);
    return () => {
      window.removeEventListener('mouseup', up);
    };
  }, [dragging]);
  const quoteKey = (e: KeyboardEvent): boolean => {
    if (sel === null) return false;
    if (e.key === 'Escape' || ((e.key === 'ArrowUp' || e.key === 'ArrowDown') && !e.shiftKey)) {
      setSel(null);
      if (e.key === 'Escape') e.preventDefault();
      return e.key === 'Escape';
    }
    const qs = quotesOf(paths, fileLines, rows, sel);
    if (e.key === 'c' && e.metaKey) {
      e.preventDefault();
      void (window as Partial<Window>).bise?.copy(qs.map((q) => q.text).join('\n'));
      return true;
    }
    // tab: the quote alone, nothing typed (the TUI's key, main m_12004)
    const tab = e.key === 'Tab' && !e.shiftKey;
    if ((!tab && e.key.length !== 1) || e.metaKey || e.ctrlKey || e.altKey || qs.length === 0) return false;
    e.preventDefault();
    handQuotes({ project, agent: to, from, quotes: qs, key: tab ? '' : e.key });
    router.go({ screen: 'thread', project, agent: to });
    return true;
  };
  // the pop's parts (lead m_11968): the ask words quote with nothing typed, '⌘C copy' copies
  const askClick = () => {
    if (sel === null) return;
    const qs = quotesOf(paths, fileLines, rows, sel);
    if (qs.length === 0) return;
    handQuotes({ project, agent: to, from, quotes: qs, key: '' });
    router.go({ screen: 'thread', project, agent: to });
  };
  const copyClick = () => {
    if (sel !== null) copyText(quotesOf(paths, fileLines, rows, sel).map((q) => q.text).join('\n'));
  };
  const onKey = (e: KeyboardEvent) => {
    if (quoteKey(e)) return;
    if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && e.shiftKey && rows.length > 0) {
      e.preventDefault();
      const d = e.key === 'ArrowDown' ? 1 : -1;
      setSel((s) => {
        if (s === null) {
          const i = (starts[Math.max(0, at.current)] ?? 0) + 1;
          return { anchor: Math.min(i, rows.length - 1), head: Math.min(i, rows.length - 1) };
        }
        return { ...s, head: Math.max(0, Math.min(rows.length - 1, s.head + d)) };
      });
      return;
    }
    const files = body.current?.querySelectorAll<HTMLElement>('.rv-file') ?? [];
    if (e.key === 'Tab' && files.length > 0) {
      e.preventDefault();
      at.current = (at.current + (e.shiftKey ? -1 : 1) + files.length) % files.length;
      files[at.current]?.scrollIntoView({ block: 'start' });
    } else if ((e.key === 'j' || e.key === 'k') && !e.metaKey && !e.ctrlKey && !e.altKey) {
      e.preventDefault();
      body.current?.scrollBy({ top: e.key === 'j' ? 60 : -60 });
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onEscape();
    }
  };

  return (
    <div className="rv-body" data-testid={`${testId}.files`} tabIndex={-1} ref={body} onKeyDown={onKey} onMouseDown={onMouseDown} onMouseOver={onMouseOver} data-selecting={sel !== null || undefined}>
      {/* never a bare grey bar (designer m_15244): say what it waits for, or that nothing differs */}
      {diff === null && error != null ? (
        <Empty
          testId={`${testId}.failed`}
          title={pages.review_failed}
          action={onRetry && <Button kind="ghost" onClick={onRetry} testId={`${testId}.retry`}>{pages.review_retry}</Button>}
        >
          {error}
        </Empty>
      ) : diff === null ? (
        <p className="rv-wait" data-testid={`${testId}.loading`}>{loading}</p>
      ) : diff.files.length === 0 ? (
        <Empty testId={`${testId}.none`} title={none}>{diff.note ?? undefined}</Empty>
      ) : (
        <>
        {diff.cut_files != null && diff.cut_files > 0 && <p className="rv-note" data-testid={`${testId}.cut`}>{pages.review_cut_files({ n: diff.cut_files })}</p>}
        {diff.files.map((f, i) => (
          <div key={f.path} className="rv-file">
            {f.note != null && f.note !== '' && <p className="rv-note">{f.note}</p>}
            <FileDiff
              path={f.path}
              head={fileHead(f)}
              add={f.add}
              rm={f.del}
              lines={fileLines[i] ?? []}
              picked={(li) => inSel(sel, (starts[i] ?? 0) + li)}
              pop={sel !== null && !dragging && lastFileOf(sel, rows) === i ? <QuotePop to={to} testId={`${testId}.quote.pop`} onAsk={askClick} onCopy={copyClick} /> : undefined}
              testId={`${testId}.file.${i}`}
            />
          </div>
        ))}
        </>
      )}
    </div>
  );
}

interface FileDiffProps {
  readonly path: string;
  readonly head: string;
  readonly add: number;
  readonly rm: number;
  readonly lines: readonly DsLine[];
  readonly picked: (i: number) => boolean;
  readonly pop: ReactNode;
  readonly testId: string;
}

/**
 * One file's Diff, its code colored by its language (diff-syntax): drawn plain first, then
 * colored in a deferred render, one file at a time (React yields between files), so a long diff
 * shows at once and stays scrollable while it colors. A file's runs are kept until its lines change.
 */
function FileDiff({ path, head, add, rm, lines, picked, pop, testId }: FileDiffProps) {
  const colored = useDeferredValue(true, false);
  const runs = useMemo(() => (colored ? diffRuns(lines, langOfPath(path)) : null), [colored, lines, path]);
  const shown = lines.map((l, li) => {
    const r = runs?.[li];
    const sel = picked(li);
    return r == null && !sel ? l : { ...l, ...(r == null ? {} : { runs: r }), ...(sel ? { sel: true } : {}) };
  });
  return <Diff file={head} add={add} rm={rm} lines={shown} pop={pop} testId={testId} />;
}

/** Row `i` of the flat list is in the selection. */
function inSel(s: Sel | null, i: number): boolean {
  if (s === null) return false;
  const [a, b] = range(s);
  return i >= a && i <= b;
}

/** The file the selection ends in (its pop goes under it). */
function lastFileOf(s: Sel, rows: readonly RowAt[]): number | undefined {
  return rows[range(s)[1]]?.f;
}
