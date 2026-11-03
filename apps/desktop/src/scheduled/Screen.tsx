// The scheduled screen (bise fait mac: page native-mac §6, mockup 8; route scheduled, the rail's
// clock): what wakes his agents, and when. The page's name is in the window's bar; under it the lead
// (ds PageBanner: 'Scheduled', its sentence, how many run and the next run), the list's row with the
// search at its right (ds ListBar), then the tasks by project
// (ds GroupLabel), one ds ListRow each (its name and how often, then who it wakes and its next run;
// its '…' and a right click are its menu, useEntityMenu). One task opens in a 400 px pane at the
// right (under 1000 px it replaces the list, with '← scheduled'); stop asks in place there. The keys
// are the shared list keys (app/listkeys.ts) plus the TUI's r, x and esc (tui-parity m_22176); '/' and
// ⌘F focus the search (app/searchkeys.ts).
// The floating composer's room at the foot (--float-room). Owner: nm-pages-a (amb-mac's before).
import { useRef, useState, type KeyboardEvent } from 'react';
import { AppIcon, Button, Empty, GroupLabel, ListBar, ListRow, ListSearch, MoreButton, PageBanner, Press, sentenceCase } from '@/ds';
import { LiveVignette, useBannerGutter } from '@/artifacts';
import { shownName } from '@/agents/model';
import { hero } from '@/proto/words_hero.gen';
import { useListKeys } from '@/app/listkeys';
import { usePageSearchKeys } from '@/app/searchkeys';
import { useEntityMenu } from '@/app/menus/use';
import { useStore } from '@/store';
import { useKeptScroll } from '@/app/keep';
import { pinFirst, pinnedIn, usePins, type PinSet } from '@/app/pins';
import { PinMark } from '@/app/pinbutton';
import { scheduled } from '@/proto/words_scheduled.gen';
import { useAllScheduled } from './hooks';
import { ahead, asks, bySoonest, byProject, heroFacts, matches, endsWords, howOften, keyOf, nextRunWords, runsWords, wakes, type Task } from './model';
import './scheduled.css';

export interface ScheduledPorts {
  /** stop a task (HubCmd scheduled_stop {project, id}) */
  stop: (project: string, id: number) => void;
  /** run it now (none: the hub has no typed command for it yet, the button is hidden) */
  runNow?: (project: string, id: number) => void;
}

/** The screen as the shell mounts it: every project's live tasks, the hub's commands through the store. */
export function ScheduledScreen() {
  const store = useStore();
  const tasks = useAllScheduled();
  const pins = usePins('schedule');
  useKeptScroll('scheduled', () => document.querySelector<HTMLElement>('[data-testid="scheduled.scroll"]'));
  return (
    <ScheduledView
      tasks={tasks}
      pins={pins}
      ports={{
        stop: (project, id) => {
          store.command({ cmd: 'scheduled_stop', project, id } as never);
        },
      }}
    />
  );
}

/** The screen from props (tests, shots). */
export function ScheduledView({ tasks, ports, now: pinned, pins }: { tasks: readonly Task[]; ports: ScheduledPorts; now?: number; pins?: PinSet }) {
  const [opened] = useState(() => Date.now());
  const now = pinned ?? opened;
  const list = bySoonest(tasks);
  const [q, setQ] = useState('');
  const shown = q === '' ? list : list.filter((x) => matches(x, q));
  const [open, setOpen] = useState<string | null>(null);
  const [asking, setAsking] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);
  const gutter = useBannerGutter();
  const cur = list.find((x) => keyOf(x) === (open ?? '')) ?? null;
  const stop = (x: Task) => {
    ports.stop(x.project, x.t.id);
    setAsking(false);
    setOpen(null);
  };
  // the TUI's scheduled keys (scheduled_screen.rs on_key, tui-parity m_22176): r run, x stop (asks);
  // esc closes the opened task, then clears the search. '/' and ⌘F focus the search (app/searchkeys.ts)
  const screenKey = (e: KeyboardEvent, k: string | null): boolean => {
    if (e.metaKey || e.ctrlKey || e.altKey) return false;
    const at = shown.find((x) => keyOf(x) === k);
    if (asking && cur) {
      if (e.key === 'Enter' || e.key === 'y') stop(cur);
      else if (e.key === 'Escape' || e.key === 'n') setAsking(false);
      return true;
    }
    const x = cur ?? at;
    if (e.key === 'Escape') {
      if (cur) setOpen(null);
      else if (q !== '') setQ('');
      else return false;
    } else if (e.key === 'x' && x) {
      setOpen(keyOf(x));
      setAsking(true);
    } else if (e.key === 'r' && ports.runNow && x) ports.runNow(x.project, x.t.id);
    else return false;
    return true;
  };
  const { sel: selKey, setSel, ref, onKeyDown, move } = useListKeys({
    order: shown.map(keyOf),
    keepAs: 'scheduled.sel',
    still: cur !== null,
    onOpen: setOpen,
    onKey: screenKey,
  });
  usePageSearchKeys(searchRef);
  // the search has the keys: ⏎ gives them back, ↑↓ give them back and move; esc clears its words, a
  // second esc gives the keys back (Artifacts' search, designer m_52674)
  const searchKey = (e: KeyboardEvent<HTMLInputElement>) => {
    const back = () => {
      e.preventDefault();
      ref.current?.focus({ preventScroll: true });
    };
    if (e.key === 'Escape') {
      if (q !== '') {
        e.preventDefault();
        setQ('');
      } else back();
    } else if (e.key === 'Enter') back();
    else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      back();
      move(e.key === 'ArrowDown' ? 1 : -1);
    }
  };
  // what he pinned leads its project's group (app/pins.ts: a stable partition)
  const isPin = (x: Task) => pins !== undefined && pinnedIn(pins, x.project, String(x.t.id));
  const groups = byProject(pinFirst(shown, isPin));
  // a click picks and opens the task, and gives the keys back to the list (a row's button keeps none)
  const pick = (x: Task) => {
    setSel(keyOf(x));
    setOpen(keyOf(x));
    setAsking(false);
    ref.current?.focus({ preventScroll: true });
  };
  return (
    <section
      ref={ref}
      className={`sch-screen${cur ? ' has-pane' : ''}`}
      data-testid="scheduled.screen"
      tabIndex={-1}
      onKeyDown={onKeyDown}
    >
      <div className="sch-main">
        {/* one scroller: the lead scrolls away with the list (designer m_39330); its foot keeps the
            floating composer's room (--float-room) */}
        <div className="sch-scroll" data-testid="scheduled.scroll">
        <PageBanner title={hero.scheduled_title} sentence={hero.scheduled_say} facts={heroFacts(list, now)}
          visual={<LiveVignette page="scheduled" />} icon="scheduled" gutter={cur ? 0 : gutter} testId="scheduled.lead" />
        {list.length > 0 && (
          // the list's own row, the search at its right (his m_52658, designer m_52674)
          <ListBar inset testId="scheduled.bar"
            end={<ListSearch ref={searchRef} value={q} onChange={(e) => { setQ(e.currentTarget.value); }} onKeyDown={searchKey} onClear={() => { setQ(''); searchRef.current?.focus(); }} testId="scheduled.search" />} />
        )}
        {list.length === 0 ? (
          <Empty testId="scheduled.empty" title={scheduled.empty_desktop} />
        ) : shown.length === 0 ? (
          <Empty testId="scheduled.none" title={scheduled.none_match_desktop} />
        ) : (
          <div data-testid="scheduled.list">
            {groups.map((g) => (
              <div key={g.project} role="group" aria-label={g.name}>
                <GroupLabel testId={`scheduled.group.${g.project}`}>{g.name}</GroupLabel>
                {g.items.map((x) => (
                  <Row key={keyOf(x)} x={x} on={keyOf(x) === selKey} now={now} pinned={isPin(x)} onClick={() => { pick(x); }} />
                ))}
              </div>
            ))}
          </div>
        )}
        </div>
      </div>
      {cur && (
        <aside className="sch-pane" data-testid="scheduled.pane">
          <Press look="sch-back" onClick={() => { setOpen(null); setAsking(false); }} data-testid="scheduled.back"><AppIcon name="back" size={14} />{scheduled.heading}</Press>
          {/* the head, then its words once (designer m_12105: no title repeating the quote) */}
          {/* its name, never its id; who it wakes dim beside it, as the TUI's opened head (designer m_15929) */}
          <div className="sch-title" data-testid="scheduled.pane.title">
            {asks(cur.t)}
            <span className="sch-k3-sub">{wakes(cur)}</span>
          </div>
          <div className="sch-label">{scheduled.words_it_sends}</div>
          <blockquote className="sch-quote" data-testid="scheduled.pane.words">{cur.t.words}</blockquote>
          <dl className="sch-kv">
            <dt>{scheduled.label_wakes}</dt><dd>{wakes(cur)}</dd>
            <dt>{scheduled.label_set_by}</dt><dd>{cur.t.by}</dd>
            <dt>{scheduled.label_how_often}</dt><dd>{howOften(cur.t)}</dd>
            <dt>{scheduled.label_next_run}</dt><dd>{nextRunWords(cur.t, now)}</dd>
            <dt>{scheduled.label_it_ends}</dt><dd>{endsWords(cur.t, now)}</dd>
            {cur.t.page != null && cur.t.page !== '' && (<><dt>{scheduled.label_keeps_fresh}</dt><dd>{cur.t.page}</dd></>)}
          </dl>
          <div className="sch-label">{scheduled.its_runs}</div>
          <div className="sch-runs">{cur.t.done === 0 ? scheduled.none_yet : runsWords(cur.t)}</div>
          {asking ? (
            <div className="sch-ask" role="group" data-testid="scheduled.ask">
              <span>{scheduled.stop_ask_desktop({ agent: shownName(cur.t.agent) })}</span>
              <span className="sch-acts">
                <Button kind="danger" onClick={() => { stop(cur); }} testId="scheduled.ask.stop">{sentenceCase(scheduled.key_stop)}</Button>
                <Button kind="ghost" onClick={() => { setAsking(false); }} testId="scheduled.ask.keep">{sentenceCase(scheduled.key_keep)}</Button>
              </span>
            </div>
          ) : (
            <div className="sch-acts">
              {/* the act; its key in its tip (no key words on the button, bise fait mac §6) */}
              {ports.runNow && <Button kind="pri" onClick={() => { ports.runNow?.(cur.project, cur.t.id); }} testId="scheduled.run">{sentenceCase(scheduled.key_run_now)}</Button>}
              <Button kind="danger" onClick={() => { setAsking(true); }} testId="scheduled.stop">{sentenceCase(scheduled.key_stop)}</Button>
            </div>
          )}
        </aside>
      )}
    </section>
  );
}

/** One task's row (ds ListRow): its name and how often, then who it wakes and its next run; its '…'
 *  and a right click are its menu (useEntityMenu: stop, and what nm-menus gives a scheduled task). */
function Row({ x, on, now, pinned, onClick }: { x: Task; on: boolean; now: number; pinned: boolean; onClick: () => void }) {
  const menu = useEntityMenu({ kind: 'scheduled', project: x.project, id: x.t.id });
  const next = x.t.next_ms != null ? scheduled.next_at({ at: ahead(x.t.next_ms, now) }) : undefined;
  return (
    <div data-row={keyOf(x)}>
      <ListRow on={on} testId={`scheduled.row.${String(x.t.id)}`} tip={x.t.words} onClick={onClick} onContextMenu={menu.onContextMenu}
        state={<AppIcon name="scheduled" size={14} />}
        title={<><span className="sch-name">{asks(x.t)}</span><span className="sch-often">{howOften(x.t)}</span></>}
        // main is 'bise' in the window (designer m_43401)
        sub={next === undefined ? shownName(x.t.agent) : `${shownName(x.t.agent)} · ${next}`}
        end={<>{pinned && <PinMark testId={`scheduled.row.${String(x.t.id)}.pinned`} />}<MoreButton menu={menu} testId={`scheduled.row.${String(x.t.id)}.more`} /></>} />
    </div>
  );
}
