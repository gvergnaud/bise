// A diff's files, one ds Diff each (the review's body, and a .diff artifact's in the panel:
// try-diff-open, architect m_52963: one component for both, never a copy). Its keys (tui-parity V6):
// tab / shift+tab next / previous file, j k scroll, esc `onEscape`; V7 / L11: drag or shift+↑↓
// select whole lines, a typed key quotes them in `to`'s thread. Each file's code is colored by its
// language (diff-syntax: syntax.ts), after a first plain paint. Owner: nm-pages-b.
import { useDeferredValue, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent, type ReactNode, type RefObject } from 'react';
import { copyText, QuotePop } from '@/quote/QuotePop';
import { flatRows, handQuotes, quotesOf, range, span, type RowAt, type Sel } from './quote';
import { Button, Check, Diff, Empty, Fold, lineDigits, Press, tipAttrs, type DiffLine as DsLine } from '@/ds';
import { MAIN } from '@/app/route';
import { pages } from '@/proto/words_pages.gen';
import { panel } from '@/proto/words_panel.gen';
import { diffRuns, langOfPath } from './syntax';
import { lineRows, lineWindow, rowStarts, type Span } from './lineWindow';
import { useRouter } from '@/app/router';
import type { Diff as DiffEv } from '@/store';
import type { GuideTarget } from '@/proto';
import type { Kind } from './parts';
import { guideColumn, kindColumn, numberOf, shownCards, viewedCount, type Card, type Section, type Words } from './column';
import { useGuideState, useSectionNav, useViewed } from './columnState';
import { GuideSlot, ReviewBar } from './ReviewBar';
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
  /** the change its guide is asked for (page review-guided); none: By kind only (a .diff artifact) */
  readonly guide?: GuideTarget;
  /** what scrolls away over the column, the bar sticky under it (a PR's state and description: page pr-guided V2) */
  readonly lead?: ReactNode;
}

export function DiffFiles({ project, diff, loading, error, onRetry, none, to, from, onEscape, testId = 'review', guide, lead }: DiffFilesProps) {
  const router = useRouter();
  const body = useRef<HTMLDivElement>(null);
  const at = useRef(-1);
  useEffect(() => {
    body.current?.focus();
  }, []);
  // the column (column.ts): its sections in the mode shown, a folded one's cards left out: the
  // quote's rows, `starts` and the test ids all follow what is shown, never git's order
  const g = useGuideState(project, diff, guide, body);
  // the sections he folded; Generated starts folded (a path rule, never the guide's weight)
  const [folded, setFolded] = useState<readonly string[]>(['generated']);
  const viewed = useViewed(project);
  const sections = useMemo(() => {
    if (diff === null) return [];
    return g.mode === 'guide' && g.shown !== null ? guideColumn(diff, g.shown, folded, viewed.set, WORDS) : kindColumn(diff, folded, viewed.set, WORDS);
  }, [diff, g.mode, g.shown, folded, viewed.set]);
  const cards = useMemo(() => shownCards(sections), [sections]);
  const fileLines = useMemo(() => cards.map((c) => c.lines), [cards]);
  const paths = useMemo(() => cards.map((c) => c.path), [cards]);
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
  // a guide part's 'Ask <agent>' (page pr-guided): its title and files:lines in the composer, never sent
  const askPart = (s: Section) => {
    handQuotes({ project, agent: to, from, quotes: [], key: pages.review_ask_part_text({ title: s.title ?? s.label, places: partPlaces(s).join(', ') }) });
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
    const plain = !e.metaKey && !e.ctrlKey && !e.altKey;
    if (e.key === BACKSLASH && e.metaKey) {
      e.preventDefault();
      setOutline((o) => !o);
    } else if ((e.key === 'n' || e.key === 'p') && plain) {
      e.preventDefault();
      nav.go(e.key === 'n' ? 1 : -1);
    } else if (e.key === 'v' && plain) {
      e.preventDefault();
      const c = cards[Math.max(0, at.current)];
      if (c !== undefined) markViewed(c, !c.viewed);
    } else if (e.key === 'Tab' && files.length > 0) {
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
  // opening or folding a section moves the shown rows: a selection over them goes
  const toggle = (key: string) => {
    setSel(null);
    at.current = -1;
    setFolded((f) => (f.includes(key) ? f.filter((x) => x !== key) : [...f, key]));
  };
  const markViewed = (c: Card, on: boolean) => {
    setSel(null);
    viewed.mark(c.file, on);
  };
  const fileAt = (c: Card, i: number) => (
    <div key={c.key} className="rv-file" data-path={c.path}>
      {c.note != null && c.note !== '' && <p className="rv-note">{c.note}</p>}
      <FileDiff
        path={c.path}
        head={
          <>
            {c.head}
            <span className="rv-vbox" {...tipAttrs(pages.review_viewed_tip)}>
              <Check on={c.viewed} onChange={(on) => { markViewed(c, on); }} label={pages.review_viewed} testId={`${testId}.file.${String(i)}.viewed`} />
            </span>
          </>
        }
        tip={c.short ? c.path : undefined}
        add={c.add}
        rm={c.del}
        lines={c.lines}
        picked={(li) => inSel(sel, (starts[i] ?? 0) + li)}
        pop={sel !== null && !dragging && lastFileOf(sel, rows) === i ? <QuotePop to={to} testId={`${testId}.quote.pop`} onAsk={askClick} onCopy={copyClick} /> : undefined}
        scroller={body}
        testId={`${testId}.file.${String(i)}`}
      />
    </div>
  );
  const cardAt = new Map(cards.map((c, i) => [c.key, i]));
  const blocksOf = (s: Section) =>
    s.blocks.map((b, k) => (
      <div key={b.folder ?? String(k)} className="rv-group">
        {b.folder !== null && <p className="rv-folder">{b.folder}</p>}
        {b.cards.map((c) => fileAt(c, cardAt.get(c.key) ?? -1))}
      </div>
    ));
  const nav = useSectionNav(body, sections);
  const [outline, setOutline] = useState(false);
  const framedNow = sections.length > 1 || (sections[0]?.blocks.length ?? 0) > 1;
  // the bar names the part he is in by its title (a narrow column: its area), one name with the outline
  const curSection = sections.find((s) => s.key === nav.cur);
  const place = [numberOf(sections, nav.cur), curSection?.title ?? curSection?.label].filter(Boolean).join(' · ');
  // narrow: the area alone, never cut for its number (‹ › already say there's more)
  const placeShort = curSection?.label ?? '';
  const curAt = sections.findIndex((s) => s.key === nav.cur);
  const viewedIn = (key: string) => new Set(sections.find((s) => s.key === key)?.blocks.flatMap((b) => b.cards.filter((c) => c.viewed).map((c) => c.file))).size;
  // Guide | By kind keeps its shape: Guide off until a guide is here; gone only on a small change
  const guideSeg = guide === undefined || g.ev?.state === 'small' ? null : g.ready ? ({ on: true } as const) : ({ on: false, why: g.ev?.state === 'writing' ? pages.review_guide_writing : pages.review_guide_none } as const);

  return (
    <div className="rv-body" data-testid={`${testId}.files`} tabIndex={-1} ref={body} onKeyDown={onKey} onMouseDown={onMouseDown} onMouseOver={onMouseOver} onScroll={(e) => { nav.onScroll(); g.noteScroll(e.currentTarget.scrollTop); }} data-selecting={sel !== null || undefined}>
      {lead}
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
        {(framedNow || guide !== undefined) && (
          <ReviewBar
            place={place}
            placeShort={placeShort}
            single={sections.length <= 1}
            onOutline={() => { setOutline((o) => !o); }}
            outline={outline ? { sections, number: (k) => numberOf(sections, k), current: nav.cur, viewed: viewedIn, onSection: (k) => { setOutline(false); nav.to(k); }, onClose: () => { setOutline(false); body.current?.focus(); }, testId } : null}
            onPrev={curAt > 0 ? () => { nav.go(-1); } : null}
            onNext={curAt < sections.length - 1 ? () => { nav.go(1); } : null}
            viewed={{ n: viewedCount(diff.files, viewed.set), total: diff.files.length }}
            mode={g.mode === 'guide' && g.shown !== null ? 'guide' : 'kind'}
            guide={guideSeg}
            onMode={g.setMode}
            slot={guide === undefined ? null : <GuideSlot ev={g.ev} shown={g.shown !== null && g.mode === 'guide'} onRun={g.run} onShow={g.show} testId={testId} />}
            under={g.under}
            testId={testId}
          />
        )}
        {diff.cut_files != null && diff.cut_files > 0 && <p className="rv-note" data-testid={`${testId}.cut`}>{pages.review_cut_files({ n: diff.cut_files })}</p>}
        {g.mode === 'guide' && g.shown !== null && g.shown.overview !== '' && <p className="rv-overview" data-testid={`${testId}.overview`}>{g.shown.overview}</p>}
        {!framedNow && g.shown === null
          ? cards.map((c, i) => fileAt(c, i))
          : sections.map((s) => (
              <div key={s.key} className="rv-part" data-section={s.key} data-testid={`${testId}.part.${s.key}`}>
                <SectionHead s={s} number={numberOf(sections, s.key)} onToggle={() => { toggle(s.key); }} testId={`${testId}.part.${s.key}.fold`}
                  ask={to === '' ? undefined : { who: to === MAIN ? panel.bise : to, go: () => { askPart(s); }, testId: `${testId}.part.${s.key}.ask` }} />
                {!s.folded && blocksOf(s)}
              </div>
            ))}
        </>
      )}
    </div>
  );
}

interface FileDiffProps {
  readonly path: string;
  readonly head: ReactNode;
  /** the head's tip: the full path when the head names it short */
  readonly tip?: string | undefined;
  readonly add: number;
  readonly rm: number;
  readonly lines: readonly DsLine[];
  readonly picked: (i: number) => boolean;
  readonly pop: ReactNode;
  /** the column's scroller: the lines near its view are drawn */
  readonly scroller: RefObject<HTMLElement | null>;
  readonly testId: string;
}

/**
 * One file's Diff, its code colored by its language (diff-syntax): drawn plain first, then
 * colored in a deferred render, one file at a time (React yields between files), so a long diff
 * shows at once and stays scrollable while it colors. Only its lines near the view are drawn
 * (code-viewer-width: lineWindow.ts), the rest a gap of their height; a file longer than
 * COLOR_WHOLE lines is colored block by block, the blocks the window shows, each kept until its
 * lines change.
 */
function FileDiff({ path, head, tip, add, rm, lines, picked, pop, scroller, testId }: FileDiffProps) {
  const box = useRef<HTMLDivElement>(null);
  const digits = useMemo(() => lineDigits(lines), [lines]);
  const { span, starts } = useLineWindow(box, scroller, lines, digits);
  const colored = useDeferredValue(true, false);
  const runsAt = useRunsAt(lines, path, colored, span);
  const shown = lines.slice(span.from, span.to).map((l, k) => {
    const li = span.from + k;
    const r = runsAt(li);
    const sel = picked(li);
    return r == null && !sel ? l : { ...l, ...(r == null ? {} : { runs: r }), ...(sel ? { sel: true } : {}) };
  });
  return (
    <div ref={box}>
      <Diff file={head} {...(tip === undefined ? {} : { fileTip: tip })} add={add} rm={rm} lines={shown} at={span.from} gap={{ before: starts[span.from] ?? 0, after: (starts.at(-1) ?? 0) - (starts[span.to] ?? 0) }} digits={digits} pop={pop} testId={testId} />
    </div>
  );
}

/** a file up to this many lines is colored whole (a comment over many lines stays one token) */
const COLOR_WHOLE = 2000;
/** a longer one, block by block, as many lines each */
const COLOR_BLOCK = 512;

/**
 * Line `li`'s colored runs (null: plain, or not colored yet): a file up to COLOR_WHOLE lines
 * colored whole; a longer one only in the COLOR_BLOCK blocks `span` shows.
 */
function useRunsAt(lines: readonly DsLine[], path: string, colored: boolean, span: Span): (li: number) => DsLine['runs'] {
  const lang = langOfPath(path);
  const whole = lines.length <= COLOR_WHOLE;
  const from = whole ? 0 : Math.floor(span.from / COLOR_BLOCK) * COLOR_BLOCK;
  const to = whole ? lines.length : Math.min(lines.length, Math.ceil(span.to / COLOR_BLOCK) * COLOR_BLOCK);
  const runs = useMemo(() => {
    if (!colored) return null;
    if (whole) return diffRuns(lines, lang);
    const out: ReturnType<typeof diffRuns> = [];
    for (let b = from; b < to; b += COLOR_BLOCK) out.push(...diffRuns(lines.slice(b, b + COLOR_BLOCK), lang));
    return out;
  }, [colored, whole, lines, lang, from, to]);
  return (li) => runs?.[li - from] ?? null;
}

/** the sign column's width, em (ds thread.css .b-dl: its number columns `digits` ch and --s-2 each, then this) */
const SIGN_EM = 1.4;
/** one mono column's width, px, by font (measured once each) */
const chOf = new Map<string, number>();

/** The columns of a file's code cell now, its number columns `digits` wide (0: no layout). */
function colsOf(diff: Element, digits: number): number {
  const css = getComputedStyle(diff);
  const pad = Number.parseFloat(css.getPropertyValue('--s-2')) || 0;
  let ch = chOf.get(css.font);
  if (ch === undefined) {
    const probe = document.createElement('span');
    probe.textContent = '0'.repeat(100);
    probe.style.cssText = 'position:absolute;visibility:hidden;white-space:pre';
    diff.append(probe);
    ch = probe.getBoundingClientRect().width / 100;
    probe.remove();
    if (ch > 0) chOf.set(css.font, ch);
  }
  const width = diff.clientWidth - 2 * (digits * ch + pad) - SIGN_EM * Number.parseFloat(css.fontSize);
  return ch > 0 && width > 0 ? Math.floor(width / ch) : 0;
}

/** The lines of a file to draw where its box sits in the scroller now (lineWindow.ts). */
function spanNow(diff: Element, s: HTMLElement | null, starts: readonly number[]): Span {
  const view = s?.clientHeight ?? 0;
  const top = s === null ? 0 : s.getBoundingClientRect().top - diff.getBoundingClientRect().top;
  // one row: the diff's line-height (ds .b-diff; a wrapped line takes whole rows)
  const rowH = Number.parseFloat(getComputedStyle(diff).lineHeight);
  return lineWindow({ top, view, rowH, starts });
}

interface Seen {
  readonly span: Span;
  /** the code cell's columns its rows were counted at */
  readonly cols: number;
}

/**
 * The lines of a file to draw (lineWindow.ts) and the row each starts at (a long line wraps:
 * lineRows at the code cell's columns): measured before paint after each render (a section folded
 * above moves it with no scroll), again on each scroll (once a frame) and resize of `scroller`. A
 * new width counts the rows again first, then places the window. Its state moves only when they do.
 */
function useLineWindow(box: RefObject<HTMLElement | null>, scroller: RefObject<HTMLElement | null>, lines: readonly DsLine[], digits: number): { span: Span; starts: readonly number[] } {
  const [seen, setSeen] = useState<Seen>({ span: { from: 0, to: 0 }, cols: 0 });
  const starts = useMemo(() => rowStarts(lines.map((l) => lineRows(l.text, seen.cols, l.head === true))), [lines, seen.cols]);
  const look = (diff: Element, s: HTMLElement | null) => {
    const cols = colsOf(diff, digits);
    const next = cols === seen.cols ? spanNow(diff, s, starts) : seen.span;
    setSeen((cur) => (cur.cols === cols && cur.span.from === next.from && cur.span.to === next.to ? cur : { span: next, cols }));
  };
  useLayoutEffect(() => {
    const diff = box.current?.firstElementChild;
    if (diff == null) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- its place and width in the scroller: known only after the commit, before paint
    look(diff, scroller.current);
  });
  useLayoutEffect(() => {
    const s = scroller.current;
    if (s === null) return undefined;
    let frame = 0;
    const later = () => {
      if (frame !== 0) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        const diff = box.current?.firstElementChild;
        if (diff != null) look(diff, s);
      });
    };
    s.addEventListener('scroll', later, { passive: true });
    const seenSize = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(later);
    seenSize?.observe(s);
    return () => {
      s.removeEventListener('scroll', later);
      seenSize?.disconnect();
      if (frame !== 0) cancelAnimationFrame(frame);
    };
  });
  return { span: seen.span, starts };
}

const kindWord: Record<Kind, string> = {
  code: pages.review_kind_code,
  tests: pages.review_kind_tests,
  docs: pages.review_kind_docs,
  generated: pages.review_kind_generated,
};

/** ⌘\\ opens the outline */
const BACKSLASH = String.fromCharCode(92);

const WORDS: Words = { kind: kindWord, files: (n) => pages.review_part_files({ n }), root: pages.review_folder_root, rest: pages.review_part_rest };

/**
 * A section's head, its fold (his: any section folds): a kind's word, or a guide part's number,
 * area and title (a critical part's title gets an ink dot and bold; a context part a dim word,
 * open: the weight never folds, architect m_57259), its summary, its facts.
 */
function SectionHead({ s, number, onToggle, testId, ask }: { s: Section; number: string | null; onToggle: () => void; testId: string; ask?: { who: string; go: () => void; testId: string } | undefined }) {
  // folded, its files' names say what it holds ('words_pages.gen.ts'): one line in the column, the
  // list ends in '…' and its tip says it whole (his m_58129: 'The rest' ran under the side panel)
  const facts = ['', s.count, s.folded ? s.names.join(', ') : '', s.sum].filter((x, k) => k === 0 || x !== '').join(' · ');
  if (s.kind !== 'part') {
    return (
      <Fold open={!s.folded} onToggle={onToggle} testId={testId}>
        <span className="rv-kind">{s.label}</span>
        <span className="rv-facts" {...tipAttrs(s.folded ? s.names.join(', ') : undefined)}>{facts}</span>
      </Fold>
    );
  }
  return (
    <>
      <Fold open={!s.folded} onToggle={onToggle} testId={testId}>
        <span className="rv-area">{[number, s.label, s.weight === 'context' ? pages.review_part_context : ''].filter(Boolean).join(' · ')}</span>
      </Fold>
      <h3 className="rv-title" data-weight={s.weight ?? undefined}>
        {s.weight === 'critical' && <i className="rv-dot" {...tipAttrs(pages.review_part_critical)} />}
        <span className="rv-title-t">{s.title}</span>
        {ask !== undefined && (
          <Press look="rv-ask" onClick={ask.go} {...tipAttrs(pages.review_ask_part_tip({ agent: ask.who }))} data-testid={ask.testId}>
            {pages.review_ask_part({ agent: ask.who })}
          </Press>
        )}
      </h3>
      {s.summary !== null && s.summary !== '' && <p className="rv-summary">{s.summary}</p>}
    </>
  );
}

/** A part's files and the lines it changes, as a quote names them: 'src/a.ts:12-30'. Pure. */
export function partPlaces(s: Pick<Section, 'blocks'>): string[] {
  return s.blocks.flatMap((b) => b.cards).map((c) => {
    const at = span(c.lines.flatMap((l) => (l.sign === '+' || l.sign === '-') && l.new != null ? [l.new] : []));
    return at === '' ? c.path : `${c.path}:${at}`;
  });
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
