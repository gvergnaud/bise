// The inbox screen's view (bise fait mac: page native-mac §6, mockup 6; designer's inbox-history
// pick A, m_24212 (2)-(6); route inbox/all, the rail's inbox): the page's name is in the window's
// bar; under the bar the banner (ds PageBanner: 'Inbox', its sentence, '3 waiting · the oldest 2 h
// ago', the accent on what waits), the list's row 24 under it (ds ListBar, his m_52658: the filters
// all / waiting / answered / withdrawn and the projects and agents menus, the search at its right;
// '/' and ⌘F focus it, app/searchkeys.ts), 'waiting for you N' first
// (ds GroupLabel; ⏎ or the chosen row's 'answer' opens the item above the composer, as ⌘I: the
// store's one answer path), then the past by day, one ds ListRow each (two lines), read-only. The
// rows' order is the hub's (Card.rank, given). No key line at the foot: the rows' tips say the keys.
// A past row's first action (his ask, inbox-open): ⏎, space or a click opens the whole item in the
// pane beside the list (Pane: the question, every option with his pick marked or his words, where
// and when, the page he answered on); the pane follows the selection (↑↓), esc closes it. The
// second: ⌘⏎ (or the pane's button, or o) goes to his answer: the thread at that very entry,
// however old, or the page at its block. The list keys are app/listkeys.ts's. At 800 the pane takes
// the list's place (past.css). The floating composer's room at the foot (--float-room). From props:
// the store's wiring is InboxScreen (InboxScreen.tsx). Owner: nm-pages-a (inbox-screen's before).
import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type RefObject } from 'react';
import type { Card } from '@/proto';
import { useListKeys } from '@/app/listkeys';
import { usePageSearchKeys } from '@/app/searchkeys';
import { AppIcon, BannerWait, Dropdown, Empty, Filters, GroupLabel, ListBar, ListRow, ListSearch, PageBanner, Press, RowAction, RowMark, tipAttrs } from '@/ds';
import { LiveVignette, useBannerGutter } from '@/artifacts';
import { hero } from '@/proto/words_hero.gen';
import { shownName } from '@/agents/model';
import { inbox } from '@/proto/words_inbox.gen';
import { idTip } from './InboxCard';
import { idOf } from './id';
import { timeOf } from '@/artifacts/model';
import { ALL, askersOf, closedMs, dayHead, heroFacts, jumpOf, jumpWords, listsOf, saidWords, STATES, waitEnd, whenWhere, type Filter, type StateFilter, type InboxRecord, type Jump, type PastRow, type WaitRow } from './past';
import { ArtText } from './artWords';
import { barFit, type BarFit } from './barfit';
import './past.css';

/** A state filter's words. */
const STATE_WORDS: Readonly<Record<StateFilter, string>> = { all: inbox.state_all, waiting: inbox.state_waiting, answered: inbox.state_answered, withdrawn: inbox.state_withdrawn };

export interface InboxPorts {
  /** ⏎ on a waiting item: it opens above the composer, as ⌘I (openInbox) */
  readonly answer: (project: string, card: number) => void;
  /** a past item: his answer in its thread (th-flash, a history step), or the page at its block */
  readonly jump: (j: Jump) => void;
  /** the page he answered on, beside (notif-artifact's open act) */
  readonly openPage?: (project: string, page: string, block?: string) => void;
}

export interface InboxViewProps {
  readonly cards: readonly Card[];
  readonly records: readonly InboxRecord[];
  /** older records wait in the hubs: called when he reaches the list's end */
  readonly onEnd?: () => void;
  /** project id → its name */
  readonly names: Readonly<Record<string, string>>;
  /** the projects, in the sidebar's order, for the menu */
  readonly projects: readonly { readonly project: string; readonly name: string }[];
  readonly ports: InboxPorts;
  /** the asker's first line after his answer, when its thread holds it (the detail's 'went on') */
  readonly nextOf?: (r: InboxRecord) => string | undefined;
  /** the record the pane shows (none: closed): the screen reads its thread meanwhile */
  readonly onDetail?: (r: InboxRecord | null) => void;
  /** a page's title and version, for the pane's chip ('↗ <title> · page · v3'; none: its id) */
  readonly pageOf?: (project: string, page: string) => PageMeta | undefined;
  /** now (a test or a shot pins it) */
  readonly now?: number;
}

export function InboxView({ cards, records, onEnd, names, projects, ports, nextOf, onDetail, pageOf, now: pinned }: InboxViewProps) {
  const [opened] = useState(() => Date.now());
  const now = pinned ?? opened;
  const [f, setF] = useState<Filter>(ALL);
  // the pane is open: it shows the selected row while that is a past one (it follows ↑↓)
  const [paned, setPaned] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);
  // the list's row as its room shrinks (the panel open beside the page: designer m_53025)
  const barRef = useRef<HTMLDivElement>(null);
  const startRef = useRef<HTMLSpanElement>(null);
  const fitAt = useBarFit(barRef, startRef);
  const gutter = useBannerGutter();
  const lists = listsOf(cards, records, names, f, now);
  const pastBy = new Map(lists.days.flatMap((d) => d.rows.map((r) => [r.key, r] as const)));
  const waitBy = new Map(lists.waiting.map((w) => [w.key, w] as const));
  // ⏎ (the first action): a waiting item opens above the composer; a past one, whole, in the pane
  const open = (key: string) => {
    const w = waitBy.get(key);
    if (w) ports.answer(w.card.project, w.card.id);
    if (pastBy.has(key)) setPaned(true);
  };
  // ⌘⏎ (the second): to his answer, in its thread at that entry or on its page
  const go = (key: string | null) => {
    const p = key === null ? undefined : pastBy.get(key);
    if (p) ports.jump(jumpOf(p.r));
    return p !== undefined;
  };
  const screenKey = (e: KeyboardEvent, k: string | null): boolean => {
    if (e.key === 'Enter' && e.metaKey && !e.ctrlKey && !e.altKey) return go(k);
    if (e.metaKey || e.ctrlKey || e.altKey) return false;
    if (cur) {
      if (e.key === 'Escape') setPaned(false);
      else if (e.key === 'o') go(cur.key);
      else if (e.key === ' ') setPaned(false);
      else return false;
      return true;
    }
    if (e.key === ' ' && k !== null && pastBy.has(k)) setPaned(true);
    else if (e.key === 'Escape' && f.q !== '') setF({ ...f, q: '' });
    else return false;
    return true;
  };
  const { sel, setSel, ref, onKeyDown, move } = useListKeys({ order: lists.order, keepAs: 'inbox.sel', onOpen: open, onKey: screenKey });
  const cur = !paned || sel === null ? undefined : pastBy.get(sel);
  const looked = cur?.r ?? null;
  useEffect(() => {
    onDetail?.(looked);
  }, [onDetail, looked]);
  usePageSearchKeys(searchRef);
  // the search has the keys: ⏎ gives them back, ↑↓ give them back and move; esc clears its words, a
  // second esc gives the keys back (Artifacts' search, designer m_52674)
  const searchKey = (e: KeyboardEvent<HTMLInputElement>) => {
    const back = () => {
      e.preventDefault();
      ref.current?.focus({ preventScroll: true });
    };
    if (e.key === 'Escape') {
      if (f.q !== '') {
        e.preventDefault();
        setF({ ...f, q: '' });
      } else back();
    } else if (e.key === 'Enter') back();
    else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      back();
      move(e.key === 'ArrowDown' ? 1 : -1);
    }
  };
  // a notification's number in the search ('567', '#567'): its row selected at once, a past one open
  // in the pane; a waiting one waits for ⏎ ('56' on the way to '567' never answers anything)
  const search = (q: string) => {
    const g = { ...f, q };
    setF(g);
    const id = idOf(q);
    if (id === null) return;
    const l = listsOf(cards, records, names, g, now);
    const p = l.days.flatMap((d) => d.rows).find((r) => r.r.card === id);
    const w = l.waiting.find((x) => x.card.id === id);
    if (p !== undefined) {
      setSel(p.key);
      setPaned(true);
    } else if (w !== undefined) setSel(w.key);
  };
  const set = (g: Partial<Filter>) => {
    setF({ ...f, ...g });
  };
  const askers = askersOf(cards, records);
  const projGroup = { id: 'projects', rows: [
      { id: 'p:', name: inbox.all_projects, current: f.project === '', testId: 'inbox.projects.all' },
      ...projects.map((p) => ({ id: `p:${p.project}`, name: p.name !== '' ? p.name : p.project, current: f.project === p.project, testId: `inbox.projects.${p.project}` })),
    ] };
  const agentGroup = { id: 'agents', rows: [
      { id: 'a:', name: inbox.all_agents, current: f.agent === '', testId: 'inbox.agents.all' },
      ...askers.map((a) => ({ id: `a:${a}`, name: shownName(a), current: f.agent === a, testId: `inbox.agents.${a}` })),
    ] };
  const facts = heroFacts(cards, now);
  const empty = cards.length === 0 && records.length === 0;
  const none = lists.order.length === 0;
  // a click picks the row and gives the keys back to the list (a row's button keeps none: listkeys)
  const pick = (key: string) => {
    setSel(key);
    ref.current?.focus({ preventScroll: true });
  };
  return (
    <section ref={ref} tabIndex={-1} className="ip-screen" data-testid="inbox.screen" onKeyDown={onKeyDown}>
      {/* one scroller: the lead and the filters scroll away with the list (designer m_39330); its
          foot keeps the floating composer's room (--float-room) */}
      <div className="ip-scroll" data-testid="inbox.scroll"
        onScroll={onEnd === undefined ? undefined : (e) => {
          const el = e.currentTarget;
          if (el.scrollHeight - el.scrollTop - el.clientHeight < 240) onEnd();
        }}>
      <PageBanner title={hero.inbox_title} sentence={hero.inbox_say} facts={<>{facts.wait !== undefined && <><BannerWait>{facts.wait}</BannerWait>{' · '}</>}{facts.rest}</>}
        visual={<LiveVignette page="inbox" />} icon="inbox" gutter={gutter} testId="inbox.lead" />
      <ListBar inset testId="inbox.filters" ref={barRef} startRef={startRef} below={fitAt === 3}
        end={<ListSearch ref={searchRef} narrow={fitAt >= 1} value={f.q} onChange={(e) => { search(e.currentTarget.value); }} onKeyDown={searchKey} onClear={() => { search(''); searchRef.current?.focus(); }} testId="inbox.search" />}>
        <Filters items={STATES.map((s) => ({ id: s, label: STATE_WORDS[s] }))} on={f.state} label={inbox.filter_label}
          onPick={(id) => { set({ state: id as StateFilter }); }} testId="inbox.state" />
        {fitAt >= 2 ? (
          // narrow: the two menus are one, its rows the two lists (designer m_53025)
          <Dropdown look="pop" label={f.project === '' && f.agent === '' ? inbox.filter_menu : [f.project === '' ? '' : (names[f.project] ?? f.project), f.agent === '' ? '' : shownName(f.agent)].filter(Boolean).join(' · ')} testId="inbox.filter"
            groups={[projGroup, agentGroup]}
            onPick={(id) => { set(id.startsWith('p:') ? { project: id.slice(2) } : { agent: id.slice(2) }); }} />
        ) : (
          <>
        <Dropdown look="pop" label={f.project === '' ? inbox.all_projects : (names[f.project] ?? f.project)} testId="inbox.projects"
          groups={[projGroup]}
          onPick={(id) => { set({ project: id.slice(2) }); }} />
        <Dropdown look="pop" label={f.agent === '' ? inbox.all_agents : shownName(f.agent)} testId="inbox.agents"
          groups={[agentGroup]}
          onPick={(id) => { set({ agent: id.slice(2) }); }} />
          </>
        )}
      </ListBar>
      <div className={`ip-body${cur ? ' paned' : ''}`}>
      {empty ? (
        <Empty title={inbox.screen_empty} testId="inbox.empty" />
      ) : none ? (
        <Empty title={inbox.screen_none} testId="inbox.none" />
      ) : (
        <div className="ip-list" data-testid="inbox.list">
          {lists.waiting.length > 0 && (
            <div role="group" aria-label={inbox.group_waiting}>
              <GroupLabel count={lists.waiting.length} waits tip={hero.waiting({ n: lists.waiting.length })} testId="inbox.waiting.head">{inbox.group_waiting}</GroupLabel>
              {lists.waiting.map((w) => (
                <WaitLine key={w.key} w={w} on={w.key === sel} end={waitEnd(w, names, now)}
                  onPick={() => { pick(w.key); }} onAnswer={() => { pick(w.key); ports.answer(w.card.project, w.card.id); }} />
              ))}
            </div>
          )}
          {lists.days.map((d) => (
            <div key={d.day} role="group" aria-label={d.title}>
              <GroupLabel testId={`inbox.day.${d.day}`}>{d.title}</GroupLabel>
              {d.rows.map((p) => (
                <PastLine key={p.key} p={p} on={p.key === sel} project={names[p.r.project] ?? p.r.project}
                  onClick={() => { pick(p.key); setPaned(true); }} />
              ))}
            </div>
          ))}
        </div>
      )}
      {cur && (
        <Pane row={cur} now={now} names={names} next={nextOf?.(cur.r)} meta={pageOf}
          onJump={() => { ports.jump(jumpOf(cur.r)); }}
          onClose={() => { setPaned(false); ref.current?.focus({ preventScroll: true }); }}
          {...(ports.openPage === undefined ? {} : { onPage: ports.openPage })} />
      )}
      </div>
      </div>
    </section>
  );
}

/** A waiting row: the ask line icon in the accent (ds RowMark, never a '?' character), the asker then
 *  the question, 'project · age' alone (no '#id', as the inbox block; ui-review m_44137); the chosen
 *  one shows its 'answer' (⏎), which opens it above the composer as ⌘I. */
function WaitLine({ w, on, end, onPick, onAnswer }: { w: WaitRow; on: boolean; end: string; onPick: () => void; onAnswer: () => void }) {
  return (
    <div className="ip-row" data-row={w.key}>
      <ListRow on={on} testId={`inbox.row.${w.key}`} tip={inbox.row_tip_wait}
        state={<RowMark icon="ask" tone="accent" label={w.glyph === '¶' ? inbox.row_mark_page : inbox.row_mark_question} />}
        title={<><span className="ip-who">{w.title}</span><span className="ip-q"><ArtText text={w.question} /></span></>}
        sub={end}
        end={on ? <RowAction onClick={onAnswer} tip={inbox.row_tip_wait} testId={`inbox.row.${w.key}.answer`}>{inbox.key_answer}</RowAction> : undefined}
        onClick={onPick} />
    </div>
  );
}

/** A past row: a check (answered) or a dash (withdrawn, dim), the asker then the question, his
 *  answer or the reason, the project, when, #id. ⏎ or a click opens it whole in the pane. */
function PastLine({ p, on, project, onClick }: { p: PastRow; on: boolean; project: string; onClick: () => void }) {
  const r = p.r;
  const ms = closedMs(r);
  const answered = r.state === 'answered';
  return (
    <div className={`ip-row${answered ? '' : ' gone'}`} data-row={p.key}>
      <ListRow on={on} testId={`inbox.row.${p.key}`} tip={inbox.row_tip_past}
        state={answered ? <AppIcon name="check" size={14} /> : <AppIcon name="fold" size={14} />}
        title={<><span className="ip-who">{shownName(r.asker)}</span><span className="ip-q"><ArtText text={r.question} /></span></>}
        sub={<><span data-testid={`inbox.row.${p.key}.said`}>{r.line2}</span>{` · ${project}`}{ms > 0 && <span data-testid={`inbox.row.${p.key}.when`} {...tipAttrs(idTip(r.card))}>{` · ${timeOf(ms)}`}</span>}</>}
        onClick={onClick} />
    </div>
  );
}
/**
 * One past item, whole, in the pane beside the list (his ask, inbox-open): who asked where and
 * when, the question whole, every option with the one he picked marked (or his words as his
 * bubble), where and when he answered, the page he answered on, what the agent did next, and the
 * way to his answer (⌘⏎).
 */
export interface PageMeta {
  readonly title: string;
  readonly version: number;
}

/** The pane's page chip: '↗ size chips · page · v3' (notif-artifact's chip), its id when unknown. Pure. */
export function pageChip(page: string, meta: PageMeta | undefined): string {
  return meta === undefined ? inbox.page_chip({ page }) : inbox.page_chip_v({ title: meta.title !== '' ? meta.title : page, v: meta.version });
}

function Pane({ row, now, names, next, meta, onJump, onClose, onPage }: {
  row: PastRow; now: number; names: Readonly<Record<string, string>>; next: string | undefined;
  meta: ((project: string, page: string) => PageMeta | undefined) | undefined;
  onJump: () => void; onClose: () => void; onPage?: (project: string, page: string, block?: string) => void;
}) {
  const r = row.r;
  const j = jumpOf(r);
  const said = saidWords(r);
  const picked = pickedOf(r);
  const page = r.where?.kind === 'page' ? r.where : null;
  return (
    <aside className="ip-pane" aria-label={inbox.detail_label({ agent: shownName(r.asker) })} data-testid="inbox.detail">
      <div className="ip-p-head">
        <Press look="ip-crumb" onClick={onClose} data-testid="inbox.detail.back"><AppIcon name="back" size={14} />{inbox.head}</Press>
        <span className="ip-d-k">{inbox.detail_asked({ agent: shownName(r.asker), project: names[r.project] ?? r.project, when: whenOf(r.asked_ms, now) })}</span>
        <Press look="ip-p-close" onClick={onClose} aria-label={inbox.detail_close_label} data-tip={inbox.detail_close_tip} data-testid="inbox.detail.close"><AppIcon name="close" size={16} /></Press>
      </div>
      <div className="ip-p-body">
        <div className="ip-d-q" data-testid="inbox.detail.question"><ArtText text={r.question} /></div>
        {r.options.length > 0 && (
          <ol className="ip-d-opts" data-testid="inbox.detail.options">
            {r.options.map((o, i) => (
              <li key={`${String(i)}:${o}`} className={picked === i ? 'on' : undefined} data-testid={`inbox.detail.option.${String(i + 1)}`} aria-current={picked === i ? 'true' : undefined}>
                <span className="ip-d-n">{picked === i ? <AppIcon name="check" size={14} /> : i + 1}</span>{o}
              </li>
            ))}
          </ol>
        )}
        {r.state === 'answered'
          ? picked === null && said !== '' && <div className="ip-d-said" data-testid="inbox.detail.said">{said}</div>
          : <div className="ip-d-gone">{r.line2}</div>}
        <div className="ip-d-k" data-testid="inbox.detail.where">{whenWhere(r, now)}</div>
        {page !== null && (
          <Press look="ip-d-chip" onClick={() => { onPage?.(r.project, page.page, page.block); }} data-testid="inbox.detail.page">
            {pageChip(page.page, meta?.(r.project, page.page))}
          </Press>
        )}
        {next !== undefined && next !== '' && <div className="ip-d-next">{inbox.detail_next({ agent: shownName(j.kind === 'thread' ? j.agent : r.asker), next })}</div>}
        <Press look="ip-d-go" onClick={onJump} data-testid="inbox.detail.go">{jumpWords(j, r.state === 'answered')}<kbd>⌘⏎</kbd></Press>
      </div>
    </aside>
  );
}

/** The option he picked: its number, else the option his words are; null for his own words. Pure. */
export function pickedOf(r: InboxRecord): number | null {
  if (r.state !== 'answered') return null;
  if (r.pick_n != null && r.pick_n >= 1 && r.pick_n <= r.options.length) return r.pick_n - 1;
  const said = saidWords(r);
  const i = r.options.indexOf(said);
  return i < 0 ? null : i;
}

/** 'today 14:02' → '14:02', else 'oct 12 14:02'. */
function whenOf(ms: number, now: number): string {
  const day = dayHead(ms, now);
  return ms <= 0 ? day : day === inbox.day_today ? timeOf(ms) : `${day} ${timeOf(ms)}`;
}

/**
 * The list row's fit (barfit.ts) as its box gets narrower or wider: measured on each resize, the
 * menus' widths kept from when they were drawn (the two side by side, or the one 'Filter ⌄').
 */
function useBarFit(bar: RefObject<HTMLDivElement | null>, start: RefObject<HTMLSpanElement | null>): BarFit {
  const [fit, setFit] = useState<BarFit>(0);
  const seen = useRef({ menus: 0, filter: 0 });
  useLayoutEffect(() => {
    const el = bar.current;
    const st = start.current;
    if (el === null || st === null) return undefined;
    const measure = () => {
      const css = getComputedStyle(el);
      const room = el.clientWidth - (parseFloat(css.paddingLeft) || 0) - (parseFloat(css.paddingRight) || 0);
      const gap = parseFloat(getComputedStyle(st).columnGap) || 8;
      const [tabs, a, b] = [...st.children].map((c) => (c as HTMLElement).offsetWidth);
      if (b !== undefined && a !== undefined) seen.current.menus = a + gap + b;
      else if (a !== undefined) seen.current.filter = a;
      const next = barFit({ room, tabs: tabs ?? 0, menus: seen.current.menus, filter: seen.current.filter || seen.current.menus / 2, gap });
      setFit((f) => (f === next ? f : next));
    };
    measure();
    const ro = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    ro?.observe(el);
    return () => {
      ro?.disconnect();
    };
  }, [bar, start, fit]);
  return fit;
}
