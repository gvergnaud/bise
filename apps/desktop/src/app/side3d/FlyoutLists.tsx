// src/app/side3d · the artifacts float (mockup 13), the processes float and the scheduled float
// (one each, designer m_44975): rows of 48 with a 40 object per kind (objects.ts). Artifacts: the
// name as the pink chip, its kind and agent under it, the accent dot when new, its age. Processes:
// the command as typed, mono 12 (typed), whose and where under it ('designer · :47977 · 1 d'), '∿'
// in text-3 while it runs (the processes page's mark: green is only a diff's +, designer m_44499).
// Scheduled: a timer's hourglass, a watch's telescope, its rhythm and next time. Each row opens its thing in a tab (openTab) and
// wires its entity's menu (useEntityMenu). Owner: nm-side.
import { useRef } from 'react';
import type { Agent, Proc } from '@/proto';
import { useArtifactFront } from '@/artifacts/front';
import { rowPrimary } from '@/artifacts/Open';
import { Fragment, useState } from 'react';
import { AppIcon, Button, tipAttrs, type AppIconName } from '@/ds';
import { useStore } from '@/store';
import { common } from '@/proto/words_common.gen';
import { ago } from '@/agents/model';
import { kindOf, type Item } from '@/artifacts/model';
import { prLine } from '@/artifacts/pr';
import { isRunning, markOf, portWords, rowCmd, stateOf, urlWords, type ProcItem } from '@/running/model';
import { asks, howOften, nextWords, type Task } from '@/scheduled/model';
import { alarms } from '@/thread/wake';
import { panel } from '@/proto/words_panel.gen';
import { side } from '@/proto/words_side.gen';
import { useEntityMenu } from '../menus/use';
import { ThingPic } from './Bubbles';
import { SideRow } from './Flyout';
import { groupParts, groupWords } from './lists';
import { artifactObj, laterObj, procObj } from './objects';
import { openTab } from './ports';
import { usePortFull, type PortBoxes } from './portfit';
import { pinnedIn, procPinId, watchPinId, type PinSet } from '../pins';

/** a row's object, css px */
export const ROW_PX = 40;

const CHIP: Record<ReturnType<typeof kindOf>, AppIconName> = { pr: 'branch', page: 'file', file: 'file', shots: 'artifacts' };

/** `group`: its project's name on bise's own screen (designer m_45706: 'acme › perf'), on each row here. */
export function ArtifactRow({ it, now, group, pinned = false }: { it: Item; now: number; group?: string; pinned?: boolean }) {
  const a = it.a;
  const menu = useEntityMenu({ kind: 'artifact', project: it.project, id: a.id });
  // ⌘⏎ with the pointer on the row: its head's first button (artifacts/front.ts rowPrimary)
  const box = useRef<HTMLDivElement>(null);
  useArtifactFront(box, 'row', rowPrimary(a));
  return (
    <SideRow
      ref={box}
      pic={<ThingPic thing={{ obj: artifactObj(a.kind) }} px={ROW_PX} tile="artifacts" />}
      title={
        <span className="b-artchip s3-chip">
          <AppIcon name={CHIP[kindOf(a.kind)]} size={14} />
          <span className="b-artchip-title">{a.title}</span>
        </span>
      }
      sub={<ArtSub a={a} by={groupWords(group, a.agent)} />}
      end={
        <>
          {a.new && <span className="s3-dot" data-tone="acc" {...tipAttrs(side.new_tip)} />}
          <span className="s3-age">{ago(a.at_ms, now)}</span>
        </>
      }
      label={a.title}
      tip={a.title}
      onClick={() => {
        openTab({ kind: 'artifact', project: it.project, id: a.id });
      }}
      menu={menu}
      pin={{ kind: 'artifact', project: it.project, id: a.id, on: pinned }}
      testId={`app.flyout.artifacts.${it.project}.${a.id}`}
    />
  );
}

/**
 * An artifact row's line: its kind and maker ('page · perf'); a PR's own line (pr-integration A):
 * '#412 · approved · checks ✓ 12/12 · your merge · perf', the hub's words in the pink only when it
 * waits on him (artifacts/pr.ts), the forge's lateness dim after them.
 */
function ArtSub({ a, by }: { a: Item['a']; by: string }) {
  const l = prLine(a);
  if (l === null) return <>{`${a.kind} · ${by}`}</>;
  return (
    <span className="s3-parts" data-testid={`app.flyout.artifacts.pr.${a.id}`}>
      <span>{l.sigil}</span>
      {l.words !== '' && (
        <span className="s3-pr-words" data-tone={l.yours ? 'acc' : undefined} data-testid={`app.flyout.artifacts.pr.${a.id}.words`}>{` · ${l.words}`}</span>
      )}
      {l.late !== '' && <span {...tipAttrs(panel.pr_as_of_tip)}>{` · ${l.late}`}</span>}
      <span>{` · ${by}`}</span>
    </span>
  );
}

/** where a process listens, its age: 'perf · localhost:5173 · 2 h'; `linked`: its end shows the
 * page's link already, so the port is said once (designer m_46263: whole or dropped, never cut) */
function procSub(p: Proc, now: number, group?: string, linked = false): readonly string[] {
  const s = stateOf(p);
  const where = linked ? '' : (urlWords(p, s)?.row ?? portWords(p, s));
  return [groupWords(group, p.agent), where, ago(p.started_ms, now)].filter((x) => x !== '');
}

/** A sub line of parts, each whole or dropped from the end (the age first), never cut mid-word; the first alone takes an ellipsis (designer m_46744). */
function Parts({ parts }: { parts: readonly string[] }) {
  return (
    <span className="s3-parts">
      {parts.map((s, i) => <span key={i}>{i === 0 ? s : ` · ${s}`}</span>)}
    </span>
  );
}

/** a process's command as he typed it ('python3 -m http.server 47977'), never the system's process name; its program by name, a 'cd <its folder> &&' gone (running/model rowCmd) */
export function typed(p: Pick<Proc, 'cmd' | 'cwd'>): string {
  const c = rowCmd(p.cmd, p.cwd);
  return c.args === '' ? c.prog : `${c.prog} ${c.args}`;
}

/** a process row's command line, command and port, for the port's fit (portfit.ts) */
function procBoxes(el: HTMLElement): PortBoxes {
  return { line: el.querySelector('.s3-row-lines'), cmd: el.querySelector('.s3-row-l1'), port: el.querySelector('.s3-port') };
}

export function ProcRow({ x, now, group, pinned = false }: { x: ProcItem; now: number; group?: string; pinned?: boolean }) {
  const p = x.p;
  const menu = useEntityMenu({ kind: 'process', project: x.project, agent: p.agent, id: p.id });
  const on = isRunning(p);
  const u = on ? urlWords(p, stateOf(p)) : null;
  const row = useRef<HTMLDivElement>(null);
  // the port gives way to the command: short at rest, full on hover or focus when the command keeps 12 characters (designer m_46894)
  const fullWords = u === null ? '' : u.label.replace(/ ↗$/u, '');
  const full = usePortFull(row, procBoxes, fullWords);
  return (
    <SideRow
      ref={row}
      pic={<ThingPic thing={{ obj: procObj(p) }} px={ROW_PX} tile="terminal" />}
      title={<span className="s3-cmd">{typed(p)}</span>}
      sub={<Parts parts={procSub(p, now, group, u !== null)} />}
      end={
        <>
          {/* its port as a link that opens the page, never the row (folded-side v9, the Overview's port link) */}
          {u !== null && (
            <span className="s3-port" role="link" aria-label={u.label} {...tipAttrs(full ? undefined : u.label)} data-short={full ? undefined : ''} onClick={(e) => {
              e.stopPropagation();
              void (window as Partial<Window>).bise?.openExternal(u.url);
            }} data-testid={`app.flyout.processes.${x.project}.${p.agent}.${p.id}.port`}>
              {full ? (
                <>
                  {fullWords}
                  <AppIcon name="open" size={14} />
                </>
              ) : u.row}
            </span>
          )}
          {on && <span className="s3-wave" {...tipAttrs(side.running_tip)} data-testid={`app.flyout.processes.${x.project}.${p.agent}.${p.id}.runs`}>{markOf(stateOf(p)).glyph}</span>}
        </>
      }
      label={p.cmd}
      tip={p.cmd}
      onClick={() => {
        openTab({ kind: 'process', project: x.project, agent: p.agent, id: p.id });
      }}
      menu={menu}
      pin={{ kind: 'process', project: x.project, id: procPinId(p.agent, p.id), on: pinned }}
      testId={`app.flyout.processes.${x.project}.${p.agent}.${p.id}`}
    />
  );
}

export function TaskRow({ x, now, group, pinned = false }: { x: Task; now: number; group?: string; pinned?: boolean }) {
  const menu = useEntityMenu({ kind: 'scheduled', project: x.project, id: x.t.id });
  return (
    <SideRow
      pic={<ThingPic thing={{ obj: laterObj('timer') }} px={ROW_PX} tile="scheduled" />}
      title={asks(x.t)}
      sub={[groupWords(group, x.t.agent), howOften(x.t), nextWords(x.t, now)].join(' · ')}
      label={asks(x.t)}
      tip={x.t.words}
      onClick={() => {
        openTab({ kind: 'scheduled', project: x.project, id: x.t.id });
      }}
      menu={menu}
      pin={{ kind: 'schedule', project: x.project, id: String(x.t.id), on: pinned }}
      testId={`app.flyout.scheduled.${x.project}.${String(x.t.id)}`}
    />
  );
}

/** An agent's watches (sb wake): its telescope rows, each opening the agent. */
export function WatchRows({ project, a, now, onOpen, group, pins, order = pins, pinned }: {
  project: string; a: Agent; now: number; onOpen: (agent: string) => void; group?: string;
  /** his schedule pins (pins.ts): each watch's pin by its watchPinId */
  pins?: PinSet;
  /** the pins `pinned` splits by: `pins` with a held click as before it (pinhold.ts usePinOrder) */
  order?: PinSet;
  /** true: only its pinned watches (they lead the section); false: only the others; none: all */
  pinned?: boolean;
}) {
  const menu = useEntityMenu({ kind: 'agent', project, agent: a.name });
  const store = useStore();
  const g = groupParts(group, a.name);
  // a watch's detail under its row (folded-side v9, the Overview's): who wakes, set when, until;
  // its agent, and 'stop watching' (HubCmd wake_stop, sb wake --stop); one open at a time
  const [open, setOpen] = useState<number | null>(null);
  const isWatchPinned = (alarm: number) => pins !== undefined && pinnedIn(pins, project, watchPinId(alarm));
  const leads = (alarm: number) => order !== undefined && pinnedIn(order, project, watchPinId(alarm));
  return (
    <>
      {alarms(a, now).filter((al) => pinned === undefined || leads(al.id) === pinned).map((al) => {
        const id = `app.flyout.scheduled.${project}.${a.name}.watch.${String(al.id)}`;
        const [head = '', ...rest] = al.title.split(' · ');
        return (
          <Fragment key={al.id}>
            <SideRow
              pic={<ThingPic thing={{ obj: laterObj('watch') }} px={ROW_PX} tile="scheduled" />}
              title={g.project === '' ? a.name : <><span className="s3-group">{g.project}</span>{g.name}</>}
              sub={[al.note, al.end === '' ? '' : panel.until({ at: al.end })].filter((s) => s !== '').join(' · ')}
              label={al.title}
              tip={al.title}
              onClick={() => {
                setOpen((o) => (o === al.id ? null : al.id));
              }}
              menu={menu}
              pin={{ kind: 'schedule', project, id: watchPinId(al.id), on: isWatchPinned(al.id) }}
              testId={id}
            />
            {open === al.id && (
              <div className="s3-detail" data-testid={`${id}.detail`}>
                <span className="s3-detail-words">
                  <span>{head}</span>
                  {rest.map((x) => <span key={x}>{x}</span>)}
                </span>
                <span className="s3-detail-acts">
                  <Button kind="ghost" size="sm" onClick={() => { onOpen(a.name); }} testId={`${id}.open`}>
                    {common.open_agent({ agent: a.name })}
                  </Button>
                  <Button kind="ghost" size="sm" onClick={() => {
                    store.command({ cmd: 'wake_stop', project, id: al.id } as never);
                    setOpen(null);
                  }} testId={`${id}.stop`}>
                    {common.stop_watching}
                  </Button>
                </span>
              </div>
            )}
          </Fragment>
        );
      })}
    </>
  );
}
