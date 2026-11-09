// Test mode only: amb-web's parts mounted alone on fake data, so qa-type's checks (typing,
// accents, edit keys, pickers, chips, the spotlight, the pearl) run before a screen mounts them.
// What they send shows in a visible log (`lab.sent`, one row per send or command), the effect a
// check reads instead of a hook. Routes `lab/composer`, `lab/spotlight`, `lab/pearl` (amb-win's
// shell draws them only in test mode).
import { useCallback, useEffect, useRef, useState, type JSX } from "react";
import { Composer, type QueueSlot } from "./Composer";
import type { ComposerActions } from "./actions";
import { entryOf } from "./entry";
import type { Outgoing } from "./message";
import { segmentsOfParts } from "./partsback";
import { prefixFirst } from "./tokens";
import type {
  ArgKind,
  ComposerSources,
  DictatePort,
  DictateState,
  PickFile,
} from "./sources";
import { Spotlight, type SpotData } from "@/spotlight";
import { LivePearl, MOODS, type Mood } from "@/pearl";

const FILES: readonly PickFile[] = [
  { path: "README.md", dir: false },
  { path: "src", dir: true },
  { path: "src/app.ts", dir: false },
  { path: "src/main.rs", dir: false },
  { path: "docs/my notes", dir: true },
  { path: "docs/my notes/plan.md", dir: false },
  { path: "shots/screen.png", dir: false },
];

/** The files he picked, newest first: the fake core's pick memory (the real one is the core's,
 *  file_picked, R14). */
const labPicked: string[] = [];

/** The fake core's file search, files.rs's shape: `src/` lists that folder's children, `src/ma`
 *  searches inside it, a bare word searches every name (prefix first); a picked file goes first
 *  in its tier (prefix, then the rest), never above a better one. */
function labFiles(q: string): readonly PickFile[] {
  const cut = q.lastIndexOf("/");
  const dir = cut < 0 ? "" : q.slice(0, cut + 1);
  const name = q.slice(cut + 1);
  const parent = (f: PickFile): string =>
    f.path.slice(0, f.path.lastIndexOf("/") + 1);
  const base = (f: PickFile): string => f.path.split("/").at(-1) ?? f.path;
  const inDir =
    dir === ""
      ? FILES
      : FILES.filter(
          (f) => f.path.startsWith(dir) && f.path !== dir.slice(0, -1),
        );
  const pool = name === "" ? inDir.filter((f) => parent(f) === dir) : inDir;
  const tier = (f: PickFile): number =>
    base(f).toLowerCase().startsWith(name.toLowerCase()) ? 0 : 1;
  const mem = (f: PickFile): number =>
    labPicked.includes(f.path) ? labPicked.indexOf(f.path) : labPicked.length;
  const ranked = prefixFirst(pool, name, base);
  return ranked
    .map((f, i) => ({ f, i }))
    .sort((a, b) => tier(a.f) - tier(b.f) || mem(a.f) - mem(b.f) || a.i - b.i)
    .map((x) => x.f)
    .slice(0, 20);
}

/** The commands of bar 2.1 (with the /shortcuts alias /keys); the ones without a screen or a cmd yet are greyed. */
const COMMANDS = [
  ["new", "start an agent: /new [-w] [name:] objective", true, true],
  ["archive", "archive an agent", true, true],
  ["restore", "bring an archived agent back", true, true],
  ["isolate", "give an agent its own worktree", true, true],
  ["rename", "rename an agent", true, true],
  ["answer", "answer notification N", true, true],
  ["close", "close notification N", true, true],
  ["model", "the model of this agent", true, true],
  ["reasoning", "its reasoning effort", true, true],
  ["interrupt", "interrupt the turn", false, true],
  ["stop", "stop an agent", true, true],
  ["compact", "compact the thread", false, true],
  // proto's cost-pause rows and its /debug (rust/proto/src/commands.rs; qa-flows m_21320)
  ["pause", "pause the whole project: every turn stops, messages and timers wait, nothing is lost", false, true],
  ["resume", "resume a paused project: what waited is delivered", false, true],
  ["usage", "what the agents and models cost, today and this week", false, false],
  ["debug", "the agent in view: its raw session, or a request as its provider got it: /debug [history | completion [provider]]", false, true],
  ["quit", "quit the app (the agents keep running)", false, true],
  ["restart", "restart the core, nothing lost", false, true],
  ["update", "update bise", false, true],
  ["inbox", "the inbox", false, false],
  ["cards", "the inbox", false, false],
  ["agents", "the agents", false, false],
  ["switch", "find an agent", false, false],
  ["archived", "archived agents", false, false],
  ["diff", "review a branch", true, false],
  ["artifacts", "artifacts", false, false],
  ["scheduled", "scheduled jobs", false, false],
  ["models", "models", false, false],
  ["provider", "providers", false, false],
  ["plugins", "plugins", true, false],
  ["computer-use", "computer use", false, false],
  ["voice", "voice settings", false, false],
  ["welcome", "welcome", false, false],
  ["setup", "check the setup again", false, false],
  ["help", "the key list", false, false],
  ["shortcuts", "the key list", false, false],
  ["keys", "the key list", false, false],
  ["theme", "auto, light or dark", true, false],
  // keychain-secrets 5426328b: the TUI's /keychain (a window line the hub doesn't run: shown, greyed)
  [
    "keychain",
    "keep your keys and sign-ins in the macOS keychain: /keychain [on|off]",
    true,
    false,
  ],
] as const;

const ARG_KINDS: Partial<Record<string, readonly ArgKind[]>> = {
  model: [
    {
      words: [
        { value: "opus", desc: "the strongest" },
        { value: "sonnet", desc: "fast" },
        { value: "default", desc: "the workspace default" },
      ],
    },
  ],
  reasoning: [
    {
      words: [
        { value: "low", desc: "" },
        { value: "medium", desc: "" },
        { value: "high", desc: "" },
      ],
    },
  ],
  archive: ["agent"],
  stop: ["agent"],
  isolate: ["agent"],
  rename: ["agent", "text"],
  restore: ["archived"],
  answer: ["card", "text"],
  close: ["card", "text"],
  new: ["text"],
  // the TUI's words (commands.rs keychain_choices, picks.rs Arg::Keychain)
  keychain: [
    {
      words: [
        { value: "on", desc: "move them to the macOS keychain" },
        { value: "off", desc: "move them back to files in ~/.bise" },
      ],
    },
  ],
};

/** The words the lab's fake mic hears (qa-type's SAID). */
export const LAB_SAID = "on garde la file d'attente";

/**
 * The lab's fake mic (no core, never the real mic): once on it says LAB_SAID word by word every
 * 250 ms; stop sends the final with every word 200 ms later; cancel ends it empty; `busy` refuses
 * at once ('the mic is busy.', as the core when fn talks).
 */
function fakeMic(busy: () => boolean): DictatePort {
  const states = new Map<string, DictateState>();
  const subs = new Map<string, Set<() => void>>();
  const timers = new Map<string, number>();
  const levels = new Set<(v: number) => void>();
  const say = (v: number): void => {
    for (const fn of [...levels]) fn(v);
  };
  let n = 0;
  const put = (id: string, st: DictateState): void => {
    states.set(id, st);
    for (const fn of [...(subs.get(id) ?? [])]) fn();
  };
  const end = (id: string): void => {
    window.clearInterval(timers.get(id));
    timers.delete(id);
    say(0);
  };
  return {
    start: () => {
      n += 1;
      const id = `lab-${String(n)}`;
      if (busy()) {
        put(id, {
          text: "",
          final: true,
          cancelled: false,
          error: "the mic is busy.",
        });
        return id;
      }
      put(id, { text: "", final: false, cancelled: false, error: null });
      const words = LAB_SAID.split(" ");
      let k = 0;
      timers.set(
        id,
        window.setInterval(() => {
          say([0.35, 0.8, 0.55, 0.9, 0.4][k % 5] ?? 0.5); // a voice, then the words
          if (k >= words.length) return;
          k += 1;
          put(id, {
            text: words.slice(0, k).join(" "),
            final: false,
            cancelled: false,
            error: null,
          });
        }, 250),
      );
      return id;
    },
    stop: (id) => {
      if (states.get(id)?.final !== false) return;
      end(id);
      window.setTimeout(() => {
        put(id, { text: LAB_SAID, final: true, cancelled: false, error: null });
      }, 200);
    },
    cancel: (id) => {
      if (states.get(id)?.final !== false) return;
      end(id);
      put(id, { text: "", final: true, cancelled: true, error: null });
    },
    read: (id) => states.get(id) ?? null,
    level: (fn) => {
      levels.add(fn);
      fn(0);
      return () => {
        levels.delete(fn);
      };
    },
    subscribe: (id, fn) => {
      const set = subs.get(id) ?? new Set();
      set.add(fn);
      subs.set(id, set);
      return () => {
        set.delete(fn);
      };
    },
  };
}

let micBusy = false;
const LAB_MIC = fakeMic(() => micBusy);

/** The lab's project folder: an `@` pick is a chip by its path there (at-chips); a check writes
 *  the files it reads under its test home (window.bise.home), else none are read. */
export const LAB_ROOT = `${(window as { bise?: Window["bise"] }).bise?.home ?? "/Users/lab"}/lab-project`;

const SOURCES: ComposerSources = {
  dictate: LAB_MIC,
  root: LAB_ROOT,
  agents: [
    { name: "main", desc: "bise" },
    { name: "perf", desc: "sort_unstable in the hot query" },
    { name: "docs", desc: "the README" },
  ],
  skills: [
    { name: "review", desc: "review a change" },
    { name: "plain-language", desc: "concise replies" },
    { name: "grill-me", desc: "stress-test a plan" },
  ],
  commands: COMMANDS.map(([name, desc, args, ready]) => ({
    name: `/${name}`,
    desc,
    args,
    ready,
    argKinds: ARG_KINDS[name],
  })),
  choices: (kind, q) => {
    const lists: Partial<
      Record<typeof kind, readonly { value: string; desc: string }[]>
    > = {
      agent: [
        { value: "perf", desc: "sort_unstable in the hot query" },
        { value: "docs", desc: "the README" },
      ],
      archived: [{ value: "old-fix", desc: "archived 2 days ago" }],
      card: [{ value: "1", desc: "alerts: which threshold?" }],
    };
    return prefixFirst(lists[kind] ?? [], q, (c) => c.value);
  },
  files: (q) => Promise.resolve(labFiles(q.replace(/^"/u, ""))),
  filePicked: (path) => {
    labPicked.splice(
      0,
      labPicked.length,
      path,
      ...labPicked.filter((p) => p !== path),
    );
  },
  saveImage: (f) =>
    Promise.resolve({
      path: `/tmp/bise-lab/${f.name || "pasted.png"}`,
      name: f.name || "pasted.png",
    }),
  artifacts: [
    {
      id: "perf-notes",
      title: "Perf notes",
      kind: "page",
      agent: "perf",
      url: "http://127.0.0.1:47123/p/perf-notes",
      atMs: 1_791_100_014_000,
    },
    {
      id: "pricing-draft",
      title: "Pricing draft",
      kind: "doc",
      agent: "docs",
      url: "http://127.0.0.1:47123/a/pricing-draft",
      atMs: 1_791_090_000_000,
    },
  ],
};

/** The lab's model when its 'the model reads images' box is unticked (K4). */
const TEXT_ONLY = { ...SOURCES, noVision: "lab-text-model" };

const SPOT: SpotData = {
  projects: [
    {
      id: "telemetry",
      name: "telemetry",
      path: "/Users/me/code/telemetry",
      branch: "main",
      agents: 2,
    },
    {
      id: "shop",
      name: "shop",
      path: "/Users/me/code/shop",
      branch: "main",
      agents: 1,
    },
  ],
  agents: [
    {
      name: "perf",
      project: "telemetry",
      status: "working",
      title: "sort_unstable in the hot query",
    },
    {
      name: "alerts",
      project: "telemetry",
      status: "idle",
      title: "page on p99 regressions",
    },
    {
      name: "t1-97894",
      project: "shop",
      status: "done",
      title: "checkout copy",
    },
  ],
  cards: [
    {
      id: "c1",
      card: 1,
      agent: "alerts",
      project: "telemetry",
      kind: "question",
      text: "Which threshold for p99: 250 ms or 400 ms?",
    },
    {
      id: "c2",
      card: 2,
      agent: "t1-97894",
      project: "shop",
      kind: "approval",
      text: "t1-97894 is ready to try\n1 commit on t1-97894 · +3 −0",
    },
  ],
  front: {
    app: "Chrome",
    title: "Pricing · Acme",
    url: "https://acme.test/pricing",
  },
};

type Log = readonly string[];

function SentLog({ log }: { log: Log }): JSX.Element {
  return (
    <ol className="lab-sent" data-testid="lab.sent">
      {log.map((l, i) => (
        <li
          key={i}
          data-testid={`lab.sent.${i + 1}`}
          style={{ whiteSpace: "pre-wrap" }}
        >
          {l}
        </li>
      ))}
    </ol>
  );
}

/** The lab's stand-in for the hub's queue (issue 23): tab queues here, the turn's end sends them
 *  all in order (sb-core's user_queued_said_at_idle), the drawer's buttons act on it. */
function useLabQueue(busy: boolean, sent: (o: Outgoing) => void, actions: { current: ComposerActions | null }): { slot: QueueSlot; onQueue: (o: Outgoing) => void; onEditNewest: () => boolean } {
  const [queued, setQueued] = useState<readonly { id: number; o: Outgoing }[]>([]);
  const [deleted, setDeleted] = useState<Outgoing | null>(null);
  const nextId = useRef(1);
  const wasBusy = useRef(busy);
  useEffect(() => {
    if (wasBusy.current && !busy && queued.length > 0) {
      for (const q of queued) sent(q.o);
      setQueued([]);
    }
    wasBusy.current = busy;
  }, [busy, queued, sent]);
  const edit = (id: number): boolean => {
    const q = queued.find((x) => x.id === id);
    if (q === undefined) return false;
    setQueued((l) => l.filter((x) => x.id !== id));
    actions.current?.putBack(entryOf(q.o.parts === undefined ? [{ kind: "text", text: q.o.text }] : segmentsOfParts(q.o.parts)));
    actions.current?.focusEnd();
    return true;
  };
  return {
    onQueue: (o) => {
      const id = nextId.current++;
      setQueued((l) => [...l, { id, o }]);
    },
    onEditNewest: () => {
      const last = queued.at(-1);
      return last === undefined ? false : edit(last.id);
    },
    slot: {
      rows: queued.map((q) => ({ id: q.id, text: q.o.text, extra: "" })),
      who: "bise",
      deleted: deleted !== null,
      onSteer: (id) => {
        const q = queued.find((x) => x.id === id);
        if (q === undefined) return;
        setQueued((l) => l.filter((x) => x.id !== id));
        sent(q.o);
      },
      onEdit: (id) => {
        edit(id);
      },
      onDelete: (id) => {
        setDeleted(queued.find((x) => x.id === id)?.o ?? null);
        setQueued((l) => l.filter((x) => x.id !== id));
      },
      onUndo: () => {
        if (deleted === null) return;
        const id = nextId.current++;
        setQueued((l) => [...l, { id, o: deleted }]);
        setDeleted(null);
      },
    },
  };
}

export function ComposerLab(): JSX.Element {
  const [log, setLog] = useState<Log>([]);
  const [busy, setBusy] = useState(false);
  const actions = useRef<ComposerActions | null>(null);
  const sent = useCallback((o: Outgoing) => {
    setLog((l) => [...l, JSON.stringify(o)]);
  }, []);
  const queue = useLabQueue(busy, sent, actions);
  const [vision, setVision] = useState(true);
  const [voice, setVoice] = useState(false);
  const [busyMic, setBusyMic] = useState(false);
  return (
    <section className="lab" data-testid="lab.composer">
      <label>
        <input
          type="checkbox"
          checked={busy}
          onChange={(e) => {
            setBusy(e.currentTarget.checked);
          }}
          data-testid="lab.busy"
        />{" "}
        a turn runs
      </label>
      <label>
        <input
          type="checkbox"
          checked={vision}
          onChange={(e) => {
            setVision(e.currentTarget.checked);
          }}
          data-testid="lab.vision"
        />{" "}
        the model reads images
      </label>
      <label>
        <input
          type="checkbox"
          checked={voice}
          onChange={(e) => {
            setVoice(e.currentTarget.checked);
          }}
          data-testid="lab.voice"
        />{" "}
        voice mode
      </label>
      <label>
        <input
          type="checkbox"
          checked={busyMic}
          onChange={(e) => {
            micBusy = e.currentTarget.checked;
            setBusyMic(micBusy);
          }}
          data-testid="lab.micBusy"
        />{" "}
        the mic is busy
      </label>
      <SentLog log={log} />
      <Composer
        target="lab/main"
        busy={busy}
        sources={vision ? SOURCES : TEXT_ONLY}
        onSend={sent}
        onQueue={queue.onQueue}
        onEditNewest={queue.onEditNewest}
        queue={queue.slot}
        onReady={(a) => {
          actions.current = a;
        }}
        onCommand={(name, args) => {
          setLog((l) => [...l, JSON.stringify({ command: name, args })]);
        }}
        onStop={() => {
          setLog((l) => [...l, JSON.stringify({ stop: true })]);
        }}
      />
    </section>
  );
}

export function SpotlightLab(): JSX.Element {
  const [log, setLog] = useState<Log>([]);
  const [answer, setAnswer] = useState<string | null>(null);
  return (
    <section className="lab" data-testid="lab.spotlight">
      <SentLog log={log} />
      <Spotlight
        data={SPOT}
        answer={answer === null ? null : { text: answer, done: true }}
        onAsk={(text, parts) => {
          setLog((l) => [
            ...l,
            JSON.stringify(
              parts === undefined ? { ask: text } : { ask: text, parts },
            ),
          ]);
          setAnswer(`bise heard: ${text}`);
        }}
        onOpen={(item) => {
          setLog((l) => [...l, JSON.stringify({ open: item.id })]);
        }}
        onClose={() => {
          setLog((l) => [...l, JSON.stringify({ close: true })]);
        }}
      />
    </section>
  );
}

const MOOD_NAMES = Object.keys(MOODS) as Mood[];

export function PearlLab(): JSX.Element {
  const [mood, setMood] = useState<Mood>("rest");
  return (
    <section className="lab" data-testid="lab.pearl">
      {MOOD_NAMES.map((m) => (
        <button
          key={m}
          type="button"
          onClick={() => {
            setMood(m);
          }}
          data-testid={`lab.mood.${m}`}
        >
          {m}
        </button>
      ))}
      <LivePearl mood={mood} size={68} testId="lab.pearl.canvas" />
    </section>
  );
}
