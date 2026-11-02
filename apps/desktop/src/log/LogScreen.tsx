// /debug in the window (designer m_17266, debug-views v3): two tabs, history and completion
// (Completion.tsx). History is the agent's raw session as the TUI's rust/tui/src/logview.rs
// draws it, read-only and redacted by the hub. The hub answers `log {project, agent, before?}`
// with a page of LogItems (newest rows, oldest first, `more` when older remain; `before` is a
// row's pos); the screen opens at the end, asks the page before when he scrolls to the top,
// and keeps his place. `log {find}` answers the pos of every row of the whole history that
// holds the words: '3 of 27', n N (⌘G ⇧⌘G) go to the next one and read the pages up to it.
// One header row per entry ('#seq  time  label  head … size'), its body under it on a rail in
// the kind's colour (prose in the thread's sans at 680, tools in the code box), folds past 6
// lines ('▸ 34 lines · 2.1 KB'), turns and compactions as rules. A body the hub cut at 4 KB has
// 'show more' under it: the whole text from the hub (whole.ts), 'show less' folds it back. Keys: ⌘F or / find, f filter,
// g G top / bottom, ↑↓ j k move, space open, ⌘O open all, r reload, ⌘] or tab completion, esc
// back. Owners: amb-web, debug-cmd.
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { flushSync } from "react-dom";
import type { LogItem } from "@/proto";
import { Head, Press } from "@/ds";
import { useCoreEvent, useStore } from "@/store";
import type { Ev } from "@/store/store";
import { useRouter } from "@/app/router";
import { usePageBand } from "@/app/pagecol";
import { stepOf } from "@/app/listkeys";
import { Markdown } from "@/thread/Markdown";
import { Code } from "@/thread/Code";
import { rangesIn } from "@/thread/findranges";
import { Completion } from "./Completion";
import { CutFoot, FindLine, K, useScreenKeys } from "./parts";
import { drawn, useWhole, type WholeApi } from "./whole";
import {
  codeText,
  editDiff,
  foldLabel,
  folds,
  kindOf,
  kindsIn,
  labelOf,
  matchesIn,
  prose,
  ruleKind,
  shown,
  sizeOf,
  stepHit,
  timeOf,
  type EditDiff,
  type Kind,
} from "./model";
import "./log.css";

/** A row and its key: its pos in the history (an older page goes before it). */
interface Row {
  readonly key: number;
  readonly it: LogItem;
}
const rowOf = (it: LogItem): Row => ({ key: it.pos, it });

/** Ask the page before when he is this close to the top. */
const NEAR_TOP = 240;
/** A find asks the hub once he stops typing this long. */
const FIND_MS = 180;

export type DebugTab = "history" | "completion";

/** /debug: the head with its two tabs, and the tab shown. */
export function LogScreen({
  project,
  agent,
  tab: first = "history",
  provider,
}: {
  project: string;
  agent: string;
  tab?: DebugTab;
  provider?: string | undefined;
}) {
  const [tab, setTab] = useState<DebugTab>(first);
  const other = () => {
    setTab((t) => (t === "history" ? "completion" : "history"));
  };
  const tabOf = (t: DebugTab) => (
    <span
      className={`lg-tab${tab === t ? " on" : ""}`}
      role="tab"
      aria-selected={tab === t}
      data-testid={`log.tab.${t}`}
      onClick={() => {
        setTab(t);
      }}
    >
      {t}
    </span>
  );
  // the right side's band (its coins, its card) stays clear: the page ends left of it (app/pagecol.ts)
  const page = useRef<HTMLElement>(null);
  const band = usePageBand(page);
  return (
    <section className="lg" ref={page} style={band} data-testid="log">
      <Head
        title={`${agent} · debug`}
        sub={<span data-testid="log.sub">read-only · redacted</span>}
        end={
          <span className="lg-tabs" role="tablist">
            {tabOf("history")}
            {tabOf("completion")}
          </span>
        }
      />
      {tab === "history" ? (
        <History project={project} agent={agent} onTab={other} />
      ) : (
        <Completion
          project={project}
          agent={agent}
          provider={provider}
          onTab={other}
        />
      )}
    </section>
  );
}

function History({
  project,
  agent,
  onTab,
}: {
  project: string;
  agent: string;
  onTab: () => void;
}) {
  const store = useStore();
  const router = useRouter();
  const whole = useWhole(project, agent);
  const [rows, setRows] = useState<readonly Row[]>([]);
  const [more, setMore] = useState(false);
  const [state, setState] = useState<"loading" | "ready" | { error: string }>(
    "loading",
  );
  const [open, setOpen] = useState<ReadonlySet<number>>(new Set());
  const [cursor, setCursor] = useState<number | null>(null);
  const [hidden, setHidden] = useState<ReadonlySet<Kind>>(new Set());
  const [filtering, setFiltering] = useState(false);
  const [finding, setFinding] = useState(false);
  const [needle, setNeedle] = useState("");
  const [at, setAt] = useState(0);
  // he went to a hit of this find already (the first ⏎ goes to the current one, the next move
  // on), even when its page is still being read
  const [went, setWent] = useState(false);
  // the hub's hits for the needle (row positions in the whole history, oldest first); null
  // while it is asked
  const [hitPos, setHitPos] = useState<readonly number[] | null>([]);
  // an older page asked, not answered yet
  const [paging, setPaging] = useState(false);
  const list = useRef<HTMLDivElement>(null);
  const root = useRef<HTMLDivElement>(null);
  const field = useRef<HTMLInputElement>(null);
  // the page asked (undefined: the newest), its scroll height before (an older page keeps his place)
  const asked = useRef<{ before?: number; height: number } | null>(null);
  // the finds asked, oldest first (the hub answers in order): their needles
  const finds = useRef<string[]>([]);
  // a hit older than the rows read: the pages before are read up to it
  const want = useRef<number | null>(null);

  // the hub's command, and what was asked (no React state: the open's effect calls it)
  const send = useCallback(
    (before?: number) => {
      asked.current = {
        ...(before === undefined ? {} : { before }),
        height: list.current?.scrollHeight ?? 0,
      };
      store.command(
        before === undefined
          ? { cmd: "log", project, agent }
          : { cmd: "log", project, agent, before },
      );
    },
    [store, project, agent],
  );
  // the needle's hits in the whole history (the hub's history_cache find), once he stops typing
  useEffect(() => {
    const n = needle.trim();
    if (n === "") return;
    const t = setTimeout(() => {
      finds.current.push(n);
      store.command({ cmd: "log", project, agent, find: n });
    }, FIND_MS);
    return () => {
      clearTimeout(t);
    };
  }, [store, project, agent, needle]);
  const ask = useCallback(
    (before?: number) => {
      setPaging(before !== undefined);
      send(before);
    },
    [send],
  );
  const reveal = useCallback(
    (key: number, block: ScrollLogicalPosition = "nearest") => {
      requestAnimationFrame(() => {
        list.current
          ?.querySelector(`[data-key="${String(key)}"]`)
          ?.scrollIntoView({ block });
      });
    },
    [],
  );
  useEffect(() => {
    send();
    root.current?.focus();
  }, [send]);

  useCoreEvent(
    useCallback(
      (ev: Ev) => {
        if (
          ev.ev === "error" &&
          ev.cmd === "log" &&
          (ev.project == null || ev.project === project)
        ) {
          asked.current = null;
          setPaging(false);
          setState({ error: ev.text });
          return;
        }
        if (ev.ev !== "log" || ev.project !== project || ev.agent !== agent)
          return;
        if (ev.hits !== undefined) {
          // a find's answer: its hits only (its page is the one ending at the newest hit,
          // the rows read stay); a later needle's answer wins
          const n = finds.current.shift();
          if (finds.current.length === 0 && n !== undefined) {
            setHitPos(ev.hits);
            setAt(Math.max(0, ev.hits.length - 1));
            setWent(false);
          }
          return;
        }
        const q = asked.current;
        if (q?.before === undefined) {
          // the newest page: the rows from scratch (an open, a reload)
          setRows(ev.items.map(rowOf));
          setOpen(new Set());
          setCursor(null);
        } else {
          setRows((rs) => {
            const first = rs[0]?.it.pos;
            const older =
              first === undefined
                ? ev.items
                : ev.items.filter((o) => o.pos < first);
            return [...older.map(rowOf), ...rs];
          });
        }
        setMore(ev.more);
        setPaging(false);
        setState("ready");
      },
      [project, agent],
    ),
  );

  // the newest page opens at the end; an older one keeps the row he was reading where it was
  useLayoutEffect(() => {
    const el = list.current;
    const q = asked.current;
    if (el === null || q === null || state !== "ready") return;
    asked.current = null;
    el.scrollTop =
      q.before === undefined
        ? el.scrollHeight
        : el.scrollTop + (el.scrollHeight - q.height);
  }, [rows, state]);

  const onScroll = () => {
    const el = list.current;
    const first = rows[0]?.it.pos;
    if (el === null || !more || asked.current !== null || first === undefined)
      return;
    if (el.scrollTop < NEAR_TOP) ask(first);
  };

  // a hit older than the rows read: the next page before, until it is read (or none remain)
  useEffect(() => {
    const w = want.current;
    if (w === null || asked.current !== null) return;
    const r = rows.find((x) => x.key === w);
    if (r !== undefined) {
      want.current = null;
      setCursor(w);
      setOpen((o) => new Set(o).add(w));
      reveal(w, "center");
      return;
    }
    const first = rows[0]?.it.pos;
    if (more && first !== undefined && w < first) ask(first);
    else want.current = null;
  }, [rows, more, ask, reveal]);

  const visible = useMemo(
    () => rows.filter((r) => shown(r.it, hidden)),
    [rows, hidden],
  );
  const entries = useMemo(
    () => visible.filter((r) => r.it.rule === undefined),
    [visible],
  );
  // the hub's hits, the row keys (pos) to go to; the rows read mark theirs
  const hits = useMemo(() => hitPos ?? [], [hitPos]);
  const hitSet = useMemo(() => new Set(hits), [hits]);
  const kinds = useMemo(() => kindsIn(rows.map((r) => r.it)), [rows]);

  // the find marks only the matched words, in the selection's tint (the current match's row
  // stronger), through the CSS highlight API: the rows' DOM stays as drawn (designer m_17531)
  useLayoutEffect(() => {
    const reg = typeof CSS === "undefined" ? undefined : CSS.highlights;
    const el = list.current;
    if (reg === undefined || el === null) return;
    const all = new Highlight();
    const cur = new Highlight();
    if (needle.trim() !== "") {
      const current = hits[at];
      for (const row of el.querySelectorAll<HTMLElement>(".lg-e.hit")) {
        const mine = row.dataset.key === String(current) ? cur : all;
        for (const r of rangesIn(row, (t) => matchesIn(t, needle), needle.trim().length)) mine.add(r);
      }
    }
    reg.set("lg-find", all);
    reg.set("lg-find-at", cur);
    return () => {
      reg.delete("lg-find");
      reg.delete("lg-find-at");
    };
  }, [needle, hits, at, visible, open]);
  const isOpen = (r: Row): boolean => !folds(r.it) || open.has(r.key);

  // a hit by its place in the hub's list: there when its row is read, else the pages before
  // are read up to it (the effect above)
  const goHit = (n: number) => {
    const key = hits[n];
    if (key === undefined) return;
    setAt(n);
    setWent(true);
    if (rows.some((r) => r.key === key)) {
      setCursor(key);
      setOpen((o) => new Set(o).add(key));
      reveal(key, "center");
    } else {
      want.current = key;
      const first = rows[0]?.it.pos;
      if (more && first !== undefined && asked.current === null) ask(first);
    }
  };
  const toggle = (key: number) => {
    setOpen((o) => {
      const n = new Set(o);
      if (n.has(key)) n.delete(key);
      else n.add(key);
      return n;
    });
  };
  const toggleAll = () => {
    const foldable = rows.filter((r) => folds(r.it)).map((r) => r.key);
    setOpen((o) =>
      foldable.every((k) => o.has(k)) ? new Set() : new Set(foldable),
    );
  };
  const move = (by: number) => {
    if (entries.length === 0) return;
    // no cursor yet: a step down starts above the first row, a step up below the last (so ↓ is
    // the first, ↑ the last, home the first, end the last)
    const found = entries.findIndex((r) => r.key === cursor);
    const i = found !== -1 ? found : by > 0 ? -1 : entries.length;
    const next = entries[Math.max(0, Math.min(entries.length - 1, i + by))];
    if (next === undefined) return;
    setCursor(next.key);
    reveal(next.key);
  };
  const edge = (top: boolean) => {
    const el = list.current;
    if (el !== null) el.scrollTop = top ? 0 : el.scrollHeight;
    const r = top ? entries[0] : entries.at(-1);
    if (r !== undefined) setCursor(r.key);
  };
  const reload = () => {
    setState("loading");
    ask();
  };
  // a click on a row: the cursor there, and the keys stay the screen's (a fold's button goes away)
  const pick = (key: number) => {
    setCursor(key);
    root.current?.focus({ preventScroll: true });
  };
  // the field drawn and focused inside the key's event: what he types right after ⌘F goes in it,
  // never to the composer (a frame later, the first letters went there)
  const openFind = () => {
    flushSync(() => {
      setFinding(true);
    });
    field.current?.select();
  };
  const closeFind = () => {
    setFinding(false);
    setNeedle("");
    setHitPos([]);
    root.current?.focus();
  };

  const onKey = (e: KeyboardEvent) => {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && (e.key === "]" || e.key === "[")) {
      // the other tab; never the window's back / forward too (app/esc-up.ts on window)
      e.preventDefault();
      e.stopPropagation();
      onTab();
      return;
    }
    if (mod && e.key.toLowerCase() === "f") {
      e.preventDefault();
      openFind();
      return;
    }
    if (mod && e.key.toLowerCase() === "g") {
      e.preventDefault();
      goHit(stepHit(at, e.shiftKey ? -1 : 1, hits.length));
      return;
    }
    if (mod && e.key.toLowerCase() === "o") {
      e.preventDefault();
      toggleAll();
      return;
    }
    if (e.target === field.current) return;
    if (mod || e.altKey) return;
    const k = e.key;
    const done = () => {
      e.preventDefault();
      e.stopPropagation();
    };
    if (k === "Escape") {
      done();
      if (filtering) setFiltering(false);
      else if (finding || needle !== "") closeFind();
      else router.back();
    } else if (k === "Tab") {
      done();
      onTab();
    } else if (k === "/") {
      done();
      openFind();
    } else if (k === "n" && hits.length > 0) {
      done();
      goHit(stepHit(at, 1, hits.length));
    } else if (k === "N" && hits.length > 0) {
      done();
      goHit(stepHit(at, -1, hits.length));
    } else if (k === "f") {
      done();
      setFiltering((f) => !f);
    } else if (k === "g") {
      done();
      edge(true);
    } else if (k === "G") {
      done();
      edge(false);
    } else if (stepOf(k) !== 0) {
      // the shared list keys (app/listkeys.ts): ↑/k ↓/j, pgup/pgdn ten, home/end
      done();
      move(stepOf(k));
    } else if ((k === " " || k === "Enter") && cursor !== null) {
      done();
      toggle(cursor);
    } else if (k === "r") {
      done();
      reload();
    }
  };
  useScreenKeys(root, onKey);

  return (
    <div className="lg-pane" tabIndex={-1} ref={root} data-testid="debug.history">
      {filtering && (
        <div className="lg-filter" data-testid="log.filter">
          {kinds.map((k) => (
            <Press
              key={k}
              look={`lg-chip k-${k}${hidden.has(k) ? " off" : ""}`}
              data-testid={`log.filter.${k}`}
              aria-pressed={!hidden.has(k)}
              onClick={() => {
                setHidden((h) => {
                  const n = new Set(h);
                  if (n.has(k)) n.delete(k);
                  else n.add(k);
                  return n;
                });
              }}
            >
              {k}
            </Press>
          ))}
        </div>
      )}
      <div
        className="lg-list"
        ref={list}
        onScroll={onScroll}
        data-testid="log.list"
      >
        {more && (
          <div className="lg-more" data-testid="log.more">
            {paging ? "reading older entries…" : "↑ older entries"}
          </div>
        )}
        {state === "loading" && rows.length === 0 && (
          <div className="lg-note">reading the session log…</div>
        )}
        {typeof state === "object" && (
          <div className="lg-note err" data-testid="log.error">
            {state.error}
          </div>
        )}
        {visible.map((r) =>
          r.it.rule !== undefined ? (
            <Rule key={r.key} row={r} />
          ) : (
            <Entry
              key={r.key}
              row={r}
              agent={agent}
              open={isOpen(r)}
              at={r.key === cursor}
              hit={hitSet.has(r.key)}
              onToggle={toggle}
              onPick={pick}
              whole={whole}
            />
          ),
        )}
      </div>
      <div className="lg-keys" data-testid="log.keys">
        {finding ? (
          <FindLine
            field={field}
            needle={needle}
            at={at}
            of={hits.length}
            busy={hitPos === null}
            onNeedle={(n) => {
              setNeedle(n);
              // the hits come from the hub (the effect above): none for blank words, else asked
              setHitPos(n.trim() === "" ? [] : null);
            }}
            onStep={(by) => {
              // the first ⏎ goes to the current hit (the newest), the next ones move on
              goHit(stepHit(at, went ? by : 0, hits.length));
            }}
            onClose={closeFind}
          />
        ) : (
          <>
            <span className="lg-keyline">
              <K k="⌘F">find</K>
              <K k="⌘G">next</K>
              <K k="f">filter</K>
              <K k="g G">top / bottom</K>
              <K k="⏎">open</K>
              <K k="⌘O">open all</K>
              <K k="⌘]">completion</K>
            </span>
            <K k="esc" esc>
              back
            </K>
          </>
        )}
      </div>
    </div>
  );
}

function Rule({ row }: { row: Row }) {
  const rule = row.it.rule ?? "";
  return (
    <div
      className={`lg-rule ${ruleKind(rule)}`}
      data-key={row.key}
      data-testid={`log.rule.${String(row.it.seq)}`}
    >
      <span className="w">{rule}</span>
    </div>
  );
}

function Entry({
  row,
  agent,
  open,
  at,
  hit,
  onToggle,
  onPick,
  whole,
}: {
  row: Row;
  agent: string;
  open: boolean;
  at: boolean;
  hit: boolean;
  onToggle: (key: number) => void;
  onPick: (key: number) => void;
  whole: WholeApi;
}) {
  const { it: sent, key } = row;
  // the row as he reads it: its body whole once he asked for it (show more)
  const of = { of: "row", pos: sent.pos } as const;
  const it = sent.body === undefined ? sent : { ...sent, body: drawn(whole.ws, of, sent.body) };
  const kind = kindOf(it);
  const size = sizeOf(sent);
  const diff = editDiff(it);
  const id = `log.row.${String(key)}`;
  return (
    <div
      className={`lg-e k-${kind}${at ? " at" : ""}${hit ? " hit" : ""}`}
      data-key={key}
      data-testid={id}
      data-seq={it.seq}
      data-kind={kind}
    >
      <div
        className="lg-h"
        role="button"
        onClick={() => {
          onPick(key);
          if (folds(it)) onToggle(key);
        }}
      >
        <span className="seq">{it.seq === 0 ? "" : `#${String(it.seq)}`}</span>
        <span className="time">{timeOf(it.at_ms)}</span>
        <span className="label" data-testid={`${id}.label`}>
          {labelOf(it, agent)}
        </span>
        <span className="head" data-testid={`${id}.head`}>
          {it.head}
          {diff !== null && (
            <span className="counts" data-testid={`${id}.counts`}>
              {diff.add > 0 && <span className="ok">+{diff.add}</span>}
              {diff.rm > 0 && <span className="er">−{diff.rm}</span>}
            </span>
          )}
        </span>
        {size !== "" && <span className="size">{size}</span>}
      </div>
      {it.body !== undefined &&
        (open ? (
          <div className="lg-b" data-testid={`${id}.body`}>
            {it.meta !== undefined && it.meta !== "" && (
              <div className="meta">{it.meta}</div>
            )}
            {diff === null ? (
              <Body it={it} />
            ) : (
              <Edit it={it} diff={diff} id={id} />
            )}
            {sent.body !== undefined && <CutFoot of={of} body={sent.body} whole={whole} id={id} />}
          </div>
        ) : (
          <Press
            look="lg-fold"
            data-testid={`${id}.fold`}
            onClick={() => {
              onPick(key);
              onToggle(key);
            }}
          >
            {foldLabel(it)}
          </Press>
        ))}
    </div>
  );
}

/** An edit as the thread shows it: − / + lines in the thread's tints, the raw args one fold below. */
function Edit({ it, diff, id }: { it: LogItem; diff: EditDiff; id: string }) {
  const [args, setArgs] = useState(false);
  return (
    <>
      <div className="b-diff lg-diff" data-testid={`${id}.diff`}>
        {diff.lines.map((l, i) => (
          <div key={i} className={l.sign === "+" ? "b-dl ad" : "b-dl rm"}>
            <i>{l.sign === "+" ? "+" : "−"}</i>
            {l.text}
          </div>
        ))}
      </div>
      {args ? (
        <Body it={it} />
      ) : (
        <Press
          look="lg-fold args"
          data-testid={`${id}.args`}
          onClick={() => {
            setArgs(true);
          }}
        >
          ▸ args
        </Press>
      )}
    </>
  );
}

function Body({ it }: { it: LogItem }) {
  if (prose(it)) {
    return (
      <div className={`lg-prose${it.role === "thinking" ? " thinking" : ""}`}>
        <Markdown text={it.body?.text ?? ""} breaks={it.role === "you"} />
      </div>
    );
  }
  const { text, lang } = codeText(it);
  return (
    <pre className="lg-code">
      {lang === "text" ? <code>{text}</code> : <Code text={text} lang={lang} />}
    </pre>
  );
}
