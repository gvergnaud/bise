// The add-a-project sheet (designer's signed S11 v2 shot 19, ambient-lead m_18490): the ds Sheet,
// a modal over the dimmed window, the same from settings › projects and from the sidebar's
// '+ add a project' / ⌘N. Its head 'Add a project' and sub; the path Field and 'Choose…' (the
// system's folder panel, window.bise.pickFolder); 'Found in your folders · 2', the repos found_scan
// found, each with its Check; the Toggle 'bise may land on main' (the repo's flow mode; paths under his home as ~/code/shop); the foot:
// Cancel, and the primary that says what it adds ('Add shop', 'Add 2 projects'). Chrome in sentence
// case, bise's sentences lowercase (designer m_43052): the scan's words are states (the first
// launch says them too), the head sentence-cases them, never in capitals.
// Neither wait spins forever (his real-home try: the scan never ended, add did nothing): the scan's
// head counts the seconds, then says what it found, the core's refusal, its broken state, or that
// it didn't answer, with 'look again'; add says 'adding shop…', then closes once the project is in
// the list (onAdded gets its rows) or says why not, and the button works again. The words are
// add.ts's (pure). Every change is the core's (project_add, found_scan). Owner: amb-home.
import { useCallback, useEffect, useState } from 'react';
import type { AppCmd, ProjectRow } from '@/proto';
import { Act, Button, Check, Field, Group, Sheet, sentenceCase } from '@/ds';
import { useBroken, useCoreEvent, useFound, useFoundFolders, useProjects, type Ev } from '@/store';
import { settings } from '@/proto/words.gen';
import { Guarded } from '@/setup/Guarded';
import { addLabel, addPaths, addSay, foundSub, offers, scanSay } from './add';
import { biseHome, openFilesSettings } from './picker';

/** The clock, read only in the click handlers (when he looks again or adds). */
const clock = (): number => Date.now();

export interface AddSheetProps {
  open: boolean;
  onClose: () => void;
  send: (cmd: AppCmd) => void;
  /** the system's folder panel (none: no 'choose…') */
  pickFolder?: (() => Promise<string | null>) | undefined;
  /** his home folder, to say ~ (none: the bridge's) */
  home?: string | undefined;
  /** the core added them: their rows (the sidebar's opener shows the first) */
  onAdded?: (rows: readonly ProjectRow[]) => void;
  /** System Settings › Privacy & Security › Files & Folders (none: the bridge's) */
  filesSettings?: () => void;
}

/** The sheet; its state (ticks, typed path, the waits) starts again at each open. */
export function AddSheet(props: AddSheetProps) {
  return props.open ? <OpenSheet {...props} /> : null;
}

/** The core's refusals of one command since mount, counted (the same words twice count). */
function useRefusal(cmd: string): { n: number; text: string } | null {
  const [err, setErr] = useState<{ n: number; text: string } | null>(null);
  const on = useCallback(
    (ev: Ev) => {
      if (ev.ev === 'error' && ev.cmd === cmd) setErr((e) => ({ n: (e?.n ?? 0) + 1, text: ev.text }));
    },
    [cmd],
  );
  useCoreEvent(on);
  return err;
}

/** Now, every second while `on` (the waits' seconds and their end). */
function useSeconds(on: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!on) return;
    const t = setInterval(() => {
      setNow(Date.now());
    }, 1000);
    return () => {
      clearInterval(t);
    };
  }, [on]);
  return now;
}

interface Adding {
  paths: string[];
  before: ReadonlySet<string>;
  at: number;
  /** the refusal count when he clicked: only a newer one is this add's */
  err0: number;
}

function OpenSheet({ onClose, send, pickFolder, onAdded, home = biseHome(), filesSettings = openFilesSettings }: AddSheetProps) {
  const found = useFound();
  const folders = useFoundFolders();
  const projects = useProjects();
  const broken = useBroken();
  const scanErr = useRefusal('found_scan');
  const addErr = useRefusal('project_add');
  const [ticked, setTicked] = useState<readonly string[]>([]);
  const [typed, setTyped] = useState('');
  const [land, setLand] = useState(false);
  // the scan: asked at open (and on 'look again'); answered once a found comes after the ask
  const [scan, setScan] = useState(() => ({ at: Date.now(), found0: found, err0: 0 }));
  const [adding, setAdding] = useState<Adding | null>(null);
  const look = useCallback(() => {
    send({ cmd: 'found_scan' });
  }, [send]);
  useEffect(look, [look]);

  const list = offers(found, projects);
  const scanError = scanErr !== null && scanErr.n > scan.err0 ? scanErr.text : null;
  const scanWaits = found === scan.found0 && scanError === null && broken === null;
  const addWaits = adding !== null;
  const now = useSeconds(scanWaits || addWaits);
  const said = scanSay({ answered: found !== scan.found0, n: list.length, error: scanError, broken, ms: Math.max(0, now - scan.at) });
  const add =
    adding === null
      ? null
      : addSay({ paths: adding.paths, projects, before: adding.before, error: addErr !== null && addErr.n > adding.err0 ? addErr.text : null, broken, ms: Math.max(0, now - adding.at) });

  // added: the sheet goes, the rows are in the list (and the sidebar)
  const addedRows = add?.kind === 'added' ? add.rows : null;
  useEffect(() => {
    if (addedRows === null) return;
    onAdded?.(addedRows);
    onClose();
  }, [addedRows, onAdded, onClose]);

  // one cause, said once: the scan's head already says the broken words, the foot doesn't repeat them
  const footSay = add !== null && add.kind !== 'added' && !(add.kind === 'failed' && said.kind === 'failed' && add.text === said.text) ? add : null;
  const all = addPaths(ticked, typed);
  const busy = add?.kind === 'adding';
  // a core that can't start projects: the scan's line above says why; add waits for 'look again' (designer m_18973)
  const blocked = all.length === 0 || broken !== null;
  const go = () => {
    if (blocked || busy) return;
    for (const path of all) send(land ? { cmd: 'project_add', path, land: true } : { cmd: 'project_add', path });
    setAdding({ paths: all, before: new Set(projects.map((p) => p.project)), at: clock(), err0: addErr?.n ?? 0 });
  };
  const again = () => {
    setScan({ at: clock(), found0: found, err0: scanErr?.n ?? 0 });
    look();
  };

  const scanHead = (
    <span className="settings-add-scan" data-testid="settings.add.scan" data-state={said.kind}>
      <span className={said.kind === 'failed' ? 'err' : undefined}>{said.kind === 'failed' ? said.text : sentenceCase(said.text)}</span>
      {said.kind === 'failed' && (
        <Act onClick={again} testId="settings.add.rescan">
          {settings.add_rescan}
        </Act>
      )}
    </span>
  );

  return (
    <Sheet
      open
      onClose={onClose}
      title={settings.add_project}
      sub={settings.add_sub}
      onPrimary={go}
      testId="settings.add"
      foot={
        <>
          {footSay !== null && (
            <span className={footSay.kind === 'failed' ? 'settings-add-say err' : 'settings-add-say'} data-testid="settings.add.say" role="status">
              {footSay.text}
            </span>
          )}
          <Button kind="ghost" onClick={onClose} testId="settings.add.cancel">
            {settings.add_cancel}
          </Button>
          <Button kind="pri" disabled={blocked} state={busy ? 'loading' : undefined} onClick={go} testId="settings.add.go">
            {addLabel(all)}
          </Button>
        </>
      }
    >
      <div className="settings-add-path">
        <Field
          autoFocus
          placeholder={settings.add_path}
          value={typed}
          onChange={(e) => {
            setTyped(e.currentTarget.value);
          }}
          testId="settings.add.path"
        />
        {pickFolder && (
          <Button
            size="sm"
            onClick={() => {
              void pickFolder().then((p) => {
                if (p !== null) setTicked((t) => (t.includes(p) ? t : [...t, p]));
              });
            }}
            testId="settings.add.choose"
          >
            {settings.add_choose}
          </Button>
        )}
      </div>
      {list.length === 0 ? (
        // no rows yet (looking, nothing new, or failed): the head alone, never an empty group's bare line (designer m_19020)
        <div className="b-gt" data-testid="settings.add.found">
          {scanHead}
        </div>
      ) : (
      <Group
        title={scanHead}
        testId="settings.add.found"
        items={list.map((f) => ({
          id: f.path,
          label: f.name,
          sub: foundSub(f, now, home),
          check: (
            <Check
              on={ticked.includes(f.path)}
              label={f.name}
              onChange={(on) => {
                setTicked((t) => (on ? [...t, f.path] : t.filter((x) => x !== f.path)));
              }}
              testId={`settings.add.found.${f.name}`}
            />
          ),
        }))}
      />
      )}
      {/* Documents, Desktop, Downloads: never scanned before his click (setup-scan-tcc, designer m_64068) */}
      {folders !== null && <Guarded folders={folders} send={send} openSettings={filesSettings} testId="settings.add.guarded" />}
      <div className="settings-add-opts">
        <Group
          testId="settings.add.opts"
          items={[
            {
              id: 'land',
              label: settings.add_land,
              sub: land ? settings.add_land_on : settings.add_land_off,
              toggle: { on: land, label: settings.add_land, onChange: setLand, testId: 'settings.add.land' },
            },
          ]}
        />
      </div>
    </Sheet>
  );
}

// the sidebar's opener (amb-win wires '+ add a project' and ⌘N to it): one host in the window
const hosts = new Set<() => void>();

/** Opens the window's add sheet; false when no host is mounted (then go to settings › projects). */
export function openAddSheet(): boolean {
  for (const h of hosts) h();
  return hosts.size > 0;
}

/** Mount once in the window: the sheet openAddSheet() opens. */
export function AddSheetHost(props: Omit<AddSheetProps, 'open' | 'onClose'>) {
  const [open, setOpen] = useState(false);
  useEffect(() => {
    const h = () => {
      setOpen(true);
    };
    hosts.add(h);
    return () => {
      hosts.delete(h);
    };
  }, []);
  const close = useCallback(() => {
    setOpen(false);
  }, []);
  return <AddSheet {...props} open={open} onClose={close} />;
}
