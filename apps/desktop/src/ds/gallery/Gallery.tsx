// src/ds · the gallery: every component in each of its states, dark and light side by side, on fake
// data. It is the design system's dev route and the source of its visual check (check.mts takes one
// shot per section and theme). Sections are `data-testid="ds.gallery.<name>"`; `?only=a,b` keeps
// those sections. Pure: no store, no core.
import { useEffect, useRef, type ReactNode } from "react";
import { rasterMark, type Rgba } from "@/shared/mark";
import { Markdown } from "@/thread/Markdown";
// the thread's layout around the markdown (its code block's head, its pre), as the window draws it
import "@/thread/thread.css";
import {
  Act, Answered, Art, ArtName, Bar, BiseMark, Kiss, Board, BriefLine, Button, Capsule, Card, CardLine, Check, Chip, ComposerFrame, ComposerStill, Day, Diff, Dock, Dot, Dropdown, Empty, Err,
  Field, Fold, Group, Head, Help, IconButton, Job, Kbd, Main, Marker, Menu, MenuItem, MenuSep, Msg, Onb, Opts, Pearl, Picker, PickerSeg, PickerTrigger, Quote, Route, Row,
  SchedHead, SectionHead, Seg, Select, Side, SideView, Skel, Spot, SpotGroup, SpotRow, SpotStill, Tabs, TextArea, Thread, Thumbs, Toast, Toggle, Tool,
  Win, type Opt,
  APP_ICONS, AppIcon, BannerWait, Filters, GroupLabel, ListBar, ListRow, ListSearch, MoreButton, PageBanner, RowAction, type EntityMenu,
} from "../index";
import { BimsToggle } from "@/bims/Cube";
import { BimsSwitch } from "@/bims/Office";
import { FitChip, NameLabel, Pill } from "@/bims/chips";
import { mapRows, MapChip } from "@/bims/MapChip";
import { MAP_IDS } from "@/bims/maps/ids";
import { mapName } from "@/bims/maps/words";

const OUT = ["13  workers: [Worker; 4],", "14  queue: Bounded<64>,"].join(String.fromCharCode(10));
const OPTS: Opt[] = [{ n: 1, text: "keep 4 workers", on: true }, { n: 2, text: "go to 8" }, { n: 3, text: "ask me later", state: "hover" }];

/** every color cue of the design system, by group: the same words, one state per entry */
export const CONTRAST: Record<string, Record<string, ReactNode>> = {
  dot: { idle: <Dot lg />, wk: <Dot state="wk" lg />, nd: <Dot state="nd" lg />, ok: <Dot state="ok" lg />, er: <Dot state="er" lg />, off: <Dot state="off" lg /> },
  chip: { plain: <Chip>state</Chip>, acc: <Chip tone="acc">state</Chip>, err: <Chip tone="err">state</Chip>, ok: <Chip tone="ok">state</Chip> },
  bar: { run: <Bar pct={60} />, done: <Bar pct={60} state="done" />, err: <Bar pct={60} state="err" /> },
  help: { plain: <Help>check the key</Help>, err: <Help err>check the key</Help> },
  field: { plain: <Field defaultValue="sk-test" readOnly />, error: <Field defaultValue="sk-test" readOnly state="error" /> },
  msg: { plain: <Msg from="agent" name="search" time="10:43">the build ran</Msg>, error: <Msg from="agent" name="search" time="10:43" state="error">the build ran</Msg> },
  dock: { question: <Dock head="search" q="the build ran" />, plain: <Dock kind="plain" head="search" q="the build ran" />, err: <Dock kind="err" head="search" q="the build ran" /> },
  toast: { plain: <Toast>the build ran</Toast>, err: <Toast err>the build ran</Toast> },
  tool: { plain: <Tool>cargo build</Tool>, err: <Tool state="err">cargo build</Tool> },
  job: {
    wk: <Job state="wk" name="index" what="the build" />, nd: <Job state="nd" name="index" what="the build" />,
    ok: <Job state="ok" name="index" what="the build" />, er: <Job state="er" name="index" what="the build" />,
  },
  route: { wk: <Route state="wk" p1="main → index" pct={50} />, ok: <Route state="ok" p1="main → index" />, er: <Route state="er" p1="main → index" /> },
  diff: { plain: <Diff file="a.rs" lines={[{ old: 1, new: 1, text: "x" }]} />, add: <Diff file="a.rs" lines={[{ new: 1, sign: "+", text: "x" }]} />, rm: <Diff file="a.rs" lines={[{ old: 1, sign: "-", text: "x" }]} /> },
};

/** one answer with every markdown element the TUI colors */
export const MD_SAMPLE = [
  "## the pool, after the bench",
  "",
  "the **p95 went from 2.1 s to 0.9 s** with *eight* workers; the change is in `src/pool.rs`. details in [the run](https://bise.dev/r/42).",
  "",
  "- raised `POOL_SIZE` to 8",
  "- kept the **timeout** at 30 s",
  "",
  "1. merge it",
  "2. watch the dashboard",
  "",
  "> the bench ran on the warm cache only",
  "",
  "| step | p95 |",
  "|---|---:|",
  "| before | 2.1 s |",
  "| after | 0.9 s |",
  "",
  "```rust",
  "// the size the bench chose",
  "pub fn size() -> usize { let n: usize = 8; n.max(\"1\".len()) }",
  "```",
].join("\n");

/** the TUI's own drawing of MD_SAMPLE (rust/tui/src/markdown.rs md_lines, codeblock.rs, syntax.rs), its colors as roles */
function TuiMd() {
  const s = (role: string, extra: object = {}) => ({ color: `var(--md-${role})`, ...extra });
  const B = { fontWeight: 700 }, I = { fontStyle: "italic" };
  const code = (t: string) => <span style={s("code", B)}>{t}</span>;
  const strong = (t: string) => <span style={s("strong", B)}>{t}</span>;
  return (
    <pre className="g-tui" data-testid="ds.md.tui">
      <span style={s("heading", B)}>the pool, after the bench</span>{"\n\n"}
      the {strong("p95 went from 2.1 s to 0.9 s")} with <span style={s("em", I)}>eight</span> workers; the change is in {code("src/pool.rs")}. details in{" "}
      <span style={{ ...s("link"), textDecoration: "underline", textDecorationColor: "var(--md-link-line)" }}>the run</span>
      <span style={s("link-url")}> (https://bise.dev/r/42)</span>.{"\n\n"}
      <span style={s("bullet")}>{"  - "}</span>raised {code("POOL_SIZE")} to 8{"\n"}
      <span style={s("bullet")}>{"  - "}</span>kept the {strong("timeout")} at 30 s{"\n\n"}
      <span style={s("number")}>{"  1. "}</span>merge it{"\n"}
      <span style={s("number")}>{"  2. "}</span>watch the dashboard{"\n\n"}
      <span style={s("quote-bar")}>{"  | "}</span><span style={s("quote")}>the bench ran on the warm cache only</span>{"\n\n"}
      <span style={s("heading", B)}>step    p95</span>{"\n"}
      <span style={s("rule")}>──────  ─────</span>{"\n"}
      {"before  2.1 s\nafter   0.9 s\n\n"}
      <span style={s("code-box")}>╭─</span><span style={s("code-label")}> rust </span><span style={s("code-box")}>{"─".repeat(52)}╮</span>{"\n"}
      <span style={s("code-box")}>│ </span><span style={s("syn-comment", I)}>// the size the bench chose</span>{"\n"}
      <span style={s("code-box")}>│ </span><span style={s("syn-keyword", B)}>pub fn</span> <span style={s("syn-call")}>size</span>
      <span style={{ color: "var(--c-text-2)" }}>() -&gt; </span><span style={s("syn-type")}>usize</span> {"{ "}
      <span style={s("syn-keyword", B)}>let</span> n: <span style={s("syn-type")}>usize</span> = <span style={s("syn-number")}>8</span>; n.<span style={s("syn-call")}>max</span>(
      <span style={s("syn-string")}>"1"</span>.<span style={s("syn-call")}>len</span>()) {"}"}{"\n"}
      <span style={s("code-box")}>╰{"─".repeat(59)}╯</span>
    </pre>
  );
}

/** the menu bar's ':*' as tray.ts draws it (src/shared/mark.ts's raster), 15 pt at 2x: a template one in the bar's ink */
function TrayIcon({ turn, wait }: { turn?: number; wait?: boolean }) {
  const cv = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = cv.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx) return;
    const css = getComputedStyle(c);
    const rgb = (v: string): Rgba => {
      const m = /#(..)(..)(..)/.exec(css.getPropertyValue(v).trim());
      return m ? [parseInt(m[1] ?? "0", 16), parseInt(m[2] ?? "0", 16), parseInt(m[3] ?? "0", 16), 255] : [128, 128, 128, 255];
    };
    const ink = rgb("--p-text");
    const px = rasterMark(30, ink, wait ? rgb("--p-accent") : ink, { size: 15, turn: turn ?? 0 });
    ctx.putImageData(new ImageData(new Uint8ClampedArray(px), 30, 30), 0, 0);
  }, [turn, wait]);
  return <canvas ref={cv} width={30} height={30} style={{ width: 15, height: 15 }} />;
}

/** one entry of a section and its name under it */
function Entry({ name, children }: { name: string; children: ReactNode }) {
  return <figure className="g-entry">{children}<figcaption>{name}</figcaption></figure>;
}

/** the sections, in the reference's order; each draws in one theme */
export const SECTIONS: Record<string, () => ReactNode> = {
  // bise's presence, the living ':*' (designer's bise-presence v10): the capsule's states, the menu
  // bar's, the two kisses frame by frame, the large ':*' in mist (onboarding hello, bise's empty thread)
  presence: () => (
    <div className="g-col">
      <Capsule pearl={<BiseMark live wink={false} size={26} listening level={0.7} tip="bise: listening" />} words={<>raise the pool to 8 in <b>telemetry</b></>} typing wave="on" />
      <Capsule pearl={<BiseMark live wink={false} size={26} working arms={["run", "run", "run"]} />} words={<span className="lo">bise is on it, with 3 agents…</span>} />
      <Capsule pearl={<BiseMark live wink={false} size={26} beat={3} />} words="telemetry runs 8 workers now; the bench says p95 went from 41 ms to 18 ms." />
      <Capsule pearl={<BiseMark live wink={false} size={26} waits kiss="c1" arms={["run", "wait", "run"]} />} words={<><b>checkout</b> <span className="lo">· waiting for you</span></>} />
      <div className="g-row g-bar">
        <span>⌁</span><TrayIcon /><span className="lo">rest</span>
        <span>⌁</span><TrayIcon turn={20} /><span className="lo">working, a step</span>
        <span>⌁</span><TrayIcon wait /><span className="lo">waits for you</span>
        <span className="lo">10:42</span>
      </div>
      <div className="g-row g-kisses">
        {[1, 1.12, 1.25, 1.12, 1, 1, 1.12, 1.25, 1.12, 1].map((k, i) => (
          <span key={i} style={{ ["--k" as string]: k }}><BiseMark waits wink={false} size={56} /></span>
        ))}
      </div>
      <div className="g-row g-top">
        <div className="g-col g-center"><Pearl size={120} glyph /><span className="lo">bise is here.</span></div>
        <div className="g-col g-center"><Pearl size={120} glyph mood="need" /><span className="lo">something waits</span></div>
        <div className="g-col g-center"><Pearl size={120} glyph mood="work" /><span className="lo">working</span></div>
      </div>
    </div>
  ),
  buttons: () => (
    <>
      <div className="g-row"><Button testId="ds.btn.default">default</Button><Button kind="pri">send</Button><Button kind="ghost">later</Button><Button kind="danger">stop it</Button></div>
      <div className="g-row"><Button state="hover">hover</Button><Button state="active">active</Button><Button state="focus">focus</Button><Button state="disabled">disabled</Button><Button kind="pri" state="loading">sending</Button></div>
      <div className="g-row"><Button size="sm" testId="ds.btn.sm">restart</Button><Button size="sm" state="hover">go back</Button><Button size="sm" state="focus">start bise</Button><Button size="sm" state="disabled">restarting…</Button><Button size="sm" pressed>↗ artifacts · 5</Button><Button size="sm" kind="ghost">cancel</Button></div>
    </>
  ),
  chips: () => (
    <div className="g-row"><Chip>main.rs</Chip><Chip tone="new">new</Chip><Chip tone="err">failed</Chip><Chip tone="ok">merged</Chip><Kbd>⌘I</Kbd><Kbd>esc</Kbd></div>
  ),
  seg: () => (
    <div className="g-col">
      <Seg items={["thread", "changes", "artifacts"]} on={0} testId="ds.seg" />
      <Seg items={["all", "needs you", "done"]} on={1} hover={2} />
    </div>
  ),
  controls: () => (
    // the switch's states (designer m_44968): off, on, hover ×2, pressed, focus, disabled off/on
    <div className="g-row"><Toggle on={false} label="off" /><Toggle on label="on" /><Toggle on={false} state="hover" /><Toggle on state="hover" /><Toggle on={false} state="active" /><Toggle on state="focus" /><Toggle on={false} state="disabled" /><Toggle on state="disabled" /><Check on={false} /><Check on /></div>
  ),
  field: () => (
    <div className="g-col">
      <Field placeholder="search projects" testId="ds.field" />
      <Field defaultValue="acme/assistant" state="focus" />
      <Field defaultValue="sk-…" state="error" />
      <Help err>that key was refused: check it in your provider's console</Help>
      <Help>stored in your Keychain, never in a file</Help>
    </div>
  ),
  status: () => (
    <div className="g-col">
      <div className="g-row"><Dot state="wk" /><Dot state="nd" /><Dot state="ok" /><Dot state="er" /><Dot state="off" /><Dot /><Dot state="nd" lg /><Pearl size={22} /><Pearl size={40} /></div>
      <Bar pct={42} /><Bar /><Bar pct={100} state="done" /><Bar pct={60} state="err" />
      <Skel /><Skel />
    </div>
  ),
  empty: () => (
    <div className="g-col">
      <Empty title="nothing waits for you">bise brings what needs you here</Empty>
      <Err>the core stopped: restarting it</Err>
    </div>
  ),
  rows: () => (
    <div className="g-side">
      <Side foot="+ add a project" testId="ds.side">
        <Row kind="me" name="bise" sub="every project" on testId="ds.row.bise" />
        <SectionHead>projects</SectionHead>
        <Row kind="pj" name="telemetry" dot="nd" end="2" needs />
        <Row kind="ag" name="alerts" dot="wk" end="12 min" />
        <Row kind="ag" name="search" dot="nd" end="?" needs state="hover" />
        <Row kind="pj" name="stock" dot="ok" end="10:40" />
        <Row kind="ag" name="flash-sale" dot="ok" end="done" state="focus" />
        <Row kind="arch" name="old-import" end="2d" />
      </Side>
    </div>
  ),
  head: () => (
    <div className="g-main">
      <Main>
        <Head title="telemetry" sub="main · 3 agents" dot="nd" end={<><Chip>⌥1</Chip><Button kind="ghost">changes</Button></>} />
        <Head title="bise" sub="every project" pearl={22} />
      </Main>
    </div>
  ),
  thread: () => (
    <div className="g-main">
      <Thread testId="ds.thread">
        <Day>today</Day>
        <Msg from="you" name="you" time="10:41" via="by voice, from Chrome" context={<Chip>decode_pool.rs</Chip>}
          quote={<>1 “workers: [Worker; 4],” decode_pool.rs:13-14 · 2 lines</>}>
          why only 4 workers here?
        </Msg>
        <Msg from="bise" name="bise" time="10:41"
          after={<><Tool>read decode_pool.rs</Tool><Tool state="open" out={OUT}>grep Worker</Tool>
            <Tool state="run">cargo test -p decode</Tool><Tool state="err">cargo build: 2 errors</Tool>
            <Route state="wk" p1="telemetry · main → alerts" p2="raise the pool to 8 and bench it" p3="2 min" pct={40} />
            <Route state="ok" p1="stock · main → flash-sale" p2="replay done" p3="10:40" />
            <Art title="↗ flash sale replay" meta="stock · 10:40" isNew /></>}>
          4 was the core count of the CI box. on your Mac, <code>8</code> is safe; i asked alerts to bench it.
        </Msg>
        <Fold>4 messages between 2 agents</Fold>
        <Msg from="agent" name="alerts" time="10:42" indent state="streaming">benching 8 workers on the replay set</Msg>
        <Marker>? search asked you · it waits above the composer</Marker>
        <Answered>main answered for you: “keep the old index”</Answered>
        <Msg from="agent" name="search" time="10:43" state="error">the index build failed twice: out of disk</Msg>
      </Thread>
    </div>
  ),
  diff: () => (
    <Diff file="src/decode_pool.rs" add={2} rm={1} testId="ds.diff" pop="quote 2 lines"
      lines={[
        { old: 12, new: 12, text: "pub struct Pool {" },
        { old: 13, sign: "-", text: "    workers: [Worker; 4]," },
        { new: 13, sign: "+", text: "    workers: [Worker; 8],", sel: true },
        { new: 14, sign: "+", text: "    queue: Bounded<128>,", sel: true },
        { old: 14, new: 15, text: "}" },
      ]} />
  ),
  composer: () => (
    <div className="g-main">
      <ComposerFrame send="off" tab="you → bise · opus · high" testId="ds.composer">
        <ComposerStill placeholder="ask bise, or tell it what to do" />
      </ComposerFrame>
      <ComposerFrame send="on" tab="you → telemetry · main" focus quote={<Quote>1 “workers: [Worker; 4],” decode_pool.rs:13-14 · 2 lines</Quote>}>
        <ComposerStill text="raise it to 8 and bench it" caret />
      </ComposerFrame>
      <ComposerFrame send="stop" tab="you → alerts">
        <ComposerStill placeholder="a turn runs: what you send waits for its end" />
      </ComposerFrame>
    </div>
  ),
  dock: () => (
    <div className="g-main">
      <ComposerFrame send="off" tab="you → search"
        tabs={<Tabs testId="ds.tabs" items={[{ id: "search", text: "search", dot: "nd", on: true }, { id: "review", text: "the review", dot: "nd" }, { id: "alerts", text: "alerts" }]} />}
        dock={<Dock head="? search needs you" keys="1 of 3   ← →   esc back" q="rebuild the index from scratch, or keep the old one?"
          ctx={{ body: "the nightly build ran out of disk at 92%; the old index still answers.", origin: "search · 10:43" }}
          opts={OPTS} typeHint="or type your answer below" testId="ds.dock" />}>
        <ComposerStill placeholder="your answer" />
      </ComposerFrame>
      <Dock head="the review asks you to approve" q="merge flash-sale into main?" opts={[{ n: 1, text: "merge" }, { n: 2, text: "not yet", picked: true }]} typeHint="pick 1 or 2" />
      <Dock kind="plain" head="alerts reports" q="8 workers: 1.9× faster, no drops." />
      <Dock kind="err" head="search failed" q="out of disk on /var: 0 B free." />
      <Opts items={OPTS} />
    </div>
  ),
  cards: () => (
    <div className="g-col">
      <Card who="telemetry · alerts" rest=" needs you" k="1 of 2" q="8 workers or keep 4?" keys="1 2 pick   esc later" stacked testId="ds.card">
        <Opts items={OPTS.slice(0, 2)} />
      </Card>
      <Card plain lead="ok" who="stock · flash-sale" rest=" is done" k="10:40" q="the replay ran clean: 0 oversold.">
        <Art title="↗ flash sale replay" meta="stock · 10:40" />
      </Card>
    </div>
  ),
  spotlight: () => (
    <div className="g-col">
      <Spot input={<SpotStill placeholder="ask bise, or jump to a project" />} keys="↵ open   ⌘↵ ask bise   esc close" testId="ds.spot">
        <SpotRow glyph={<BiseMark size={14} wink={false} />} name="ask bise" what="“why only 4 workers”" hotkey="⌘⏎ ask" on />
        <SpotGroup>agents</SpotGroup>
        <SpotRow glyph="?" name="search" where="telemetry" what="needs you: rebuild the index?" hotkey="⏎ answer" />
        <SpotRow glyph="∿" name="alerts" where="telemetry" what="benching 8 workers on the staging cluster before the flash sale" hotkey="⏎ open" />
        <SpotRow glyph="✓" name="flash-sale" where="stock" what="done 10:40" />
        <SpotGroup>projects</SpotGroup>
        <SpotRow glyph="·" name="stock" what="2 agents" hotkey="⌘2" />
      </Spot>
      <Spot input={<SpotStill text="why only 4 workers" />} to="to telemetry" />
    </div>
  ),
  menus: () => (
    <div className="g-row g-top">
      <Dropdown label="all agents" testId="ds.dropdown" onPick={() => undefined} groups={[{ id: "a", rows: [
        { id: "", name: "all agents", current: true },
        ...Array.from({ length: 40 }, (_, i) => ({ id: `agent-${String(i)}`, name: `agent-${String(i)}` })),
      ] }]} />
      <PickerTrigger open>opus 5.5 high</PickerTrigger>
      <Menu testId="ds.menu">
        <MenuItem hotkey="⌘C">copy</MenuItem>
        <MenuItem hotkey="⌘⇧C" on>copy as markdown</MenuItem>
        <MenuItem disabled>quote</MenuItem>
        <MenuSep />
        <MenuItem danger>stop the agent</MenuItem>
      </Menu>
      <Picker title="mode" titleKeys="⇧⇥" testId="ds.picker.mode" onPick={() => undefined} groups={[{ id: "m", rows: [
        { id: "yolo", name: "yolo", secondary: "everything runs, nothing asks", current: true },
        { id: "auto", name: "approvals", secondary: "safe calls run, risky ones ask you" },
      ] }]} />
      <Picker testId="ds.picker.model" active="sonnet" onPick={() => undefined}
        groups={[
          { id: "a", head: "Anthropic", rows: [{ id: "opus", name: "opus 5.5", current: true }, { id: "sonnet", name: "sonnet 5" }] },
          { id: "f", head: "Anthropic (foundry proxy)", rows: [{ id: "fopus", name: "opus 5.5" }, { id: "off", name: "haiku 5", disabled: true, tip: "set it up first" }] },
        ]}
        footer={<PickerSeg head="effort" items={["low", "medium", "high", "max"]} on="high" onPick={() => undefined} />} />
      <Picker testId="ds.picker.slash" active="review" inline onPick={() => undefined} groups={[{ id: "c", rows: [
        { id: "review", glyph: "/", name: "review", secondary: "an agent reads the branch's changes and says what to fix" },
        { id: "compact", glyph: "/", name: "compact", secondary: "summarize this thread to free its context" },
        { id: "new", glyph: "/", name: "new", secondary: "start a task with its own agent", disabled: true },
      ] }]} />
      <Picker testId="ds.picker.empty" groups={[]} empty="no match" onPick={() => undefined} />
      <div className="g-col">
        <Toast lead="done" action={{ text: "open" }}>flash-sale is done</Toast>
        <Toast lead="bise">bise 2026.10.2 is ready: restart to update</Toast>
        <Toast err action={{ text: "retry" }}>the send failed: the core is offline</Toast>
        <Toast>copied</Toast>
      </div>
    </div>
  ),
  board: () => (
    <div className="g-row g-top">
      <Board testId="ds.board">
        <Job state="wk" name="alerts" hotkey="⌥1" psi end="12 min" what="bench 8 workers" pct={40} />
        <Job state="wk" name="index" end="2 min" what="rebuild the index" ind on />
        <Job state="nd" name="search" hotkey="⌥2" endChip={{ children: "?", tone: "acc" }} what="rebuild or keep?" hover />
        <Job state="er" name="import" end="▲" what="out of disk" />
        <Job state="ok" name="flash-sale" end="10:40" what="replay done" />
        <Job state="" name="docs" end="idle" what="waits for a task" />
        <Job state="sched" name="nightly" end="02:00" what="the nightly replay" />
        <Job state="loading" name="…" />
      </Board>
      <div style={{ width: 220 }}><Thumbs items={[{ id: "a", title: "flash sale replay" }, { id: "b", title: "bench chart", hover: true }]} /></div>
    </div>
  ),
  sideview: () => (
    <div className="g-row g-top">
      <SideView title="changes" end={<><Chip tone="ok">+2 −1</Chip><Button kind="ghost">open in editor</Button></>} testId="ds.sideview">
        <Row name="src/decode_pool.rs" end="+2 −1" on />
        <Row name="src/bench.rs" end="+40" />
      </SideView>
      <SideView title="artifacts" loading />
      <SideView title="versions" empty={{ title: "no versions yet", text: "the agent hasn't made any" }} />
    </div>
  ),
  settings: () => (
    // a settings group (the route's page is settings/Settings.tsx)
    <div className="b-settings">
      <Group title="Appearance" testId="ds.group" items={[
        { id: "theme", label: "theme", control: <Seg items={["system", "dark", "light"]} on={0} /> },
        { id: "sounds", label: "sounds", sub: "a soft tick when an agent needs you", toggle: { on: true, label: "sounds" } },
        { id: "dock", label: "dock badge", sub: "the count of what waits on you", toggle: { on: false, label: "dock badge" } },
        { id: "beta", label: "beta builds", sub: "managed by your organization", toggle: { on: false, label: "beta builds", state: "disabled" } },
        { id: "workspace", label: "workspace", value: "~/bise" },
        { id: "sync", label: "sync drafts", check: <Check on />, control: <Button>sync now</Button> },
      ]} help="changes apply at once" />
    </div>
  ),
  onboarding: () => (
    <Onb mark={<Kiss size={112} pop turn />} title="hold fn, then talk" sub="bise hears you only while fn is down. let go and it acts on what you said."
      fn="down" cta={{ text: "continue" }} steps={4} step={1} testId="ds.onb" />
  ),
  capsule: () => (
    <div className="g-col">
      <Capsule words={<>raise the pool to 8 in <b>telemetry</b> <span className="lo">and bench it</span></>} typing wave="on" shot
        chips={<><Chip>Chrome · grafana</Chip><Chip tone="acc">telemetry</Chip></>} hint="release fn to send   fn esc cancel" testId="ds.capsule" />
      <Capsule words="sending to telemetry…" countdown />
      <Capsule lead="off" words={<span className="lo">offline: what you say waits</span>} wave="off" />
    </div>
  ),
  // never by color alone (bar N2): each group draws the same words in each of its color states;
  // shoot.mts checks that, with increase contrast, every state of a group reads differently without
  // its color (a glyph, a word or a shape), items `ds.contrast.<group>.<state>`
  contrast: () => (
    <div className="g-col">
      {Object.entries(CONTRAST).map(([group, states]) => (
        <div key={group} className="g-row" data-testid={"ds.contrast." + group}>
          {Object.entries(states).map(([state, node]) => (
            <div key={state} className="g-cell" data-testid={"ds.contrast." + group + "." + state}>{node}</div>
          ))}
        </div>
      ))}
    </div>
  ),
  // designer's raw-controls call (m_18343): the controls a screen may not draw itself, in their states
  actions: () => (
    <>
      <div className="g-row">
        <IconButton label="sidebar" keys="⌘B" testId="ds.ibtn">☰</IconButton><IconButton label="panel" pressed>▥</IconButton>
        <IconButton label="close" state="hover">✕</IconButton><IconButton label="close" state="focus">✕</IconButton>
        <IconButton label="close" state="disabled">✕</IconButton><IconButton label="to the end" size="bar">↓</IconButton>
      </div>
      <div className="g-row">
        <span>queued · sends when bise's turn ends <Act testId="ds.act">edit</Act> · <Act>drop</Act></span>
        <Act state="hover">give it back</Act><Act state="focus">show the report</Act><Act tone="err">retry</Act><Act state="disabled">copied</Act>
      </div>
      <div className="g-row g-col">
        <TextArea defaultValue={"a brief over\ntwo lines"} placeholder="what should it do?" readOnly testId="ds.area" />
        <TextArea defaultValue="" placeholder="or type your answer" readOnly state="error" />
      </div>
      <div className="g-row g-col">
        <Select value="mistral-medium" label="model" testId="ds.select" onChange={() => undefined}
          options={[{ value: "mistral-medium", label: "Mistral Medium", hint: "default" }, { value: "mistral-small", label: "Mistral Small" }]} />
        <Select value="" placeholder="pick a model" label="model" onChange={() => undefined} options={[{ value: "a", label: "a" }]} />
      </div>
      <div className="g-row g-col">
        <Row kind="pj" name="telemetry" dot="nd" end="2" needs onClick={() => undefined} testId="ds.row.btn" />
        <Row kind="ag" name="perf" dot="wk" end="4 min" on onClick={() => undefined} />
        <Row kind="ag" name="alerts" dot="ok" end="done" state="hover" onClick={() => undefined} />
      </div>
      {/* the Sheet's own markup, drawn in place (live it is a modal over the ground at 62%, Radix Dialog) */}
      <div className="b-sheet still" data-testid="ds.sheet">
        <div className="b-sheet-head"><h2 className="t">add a project</h2><p className="sub">a folder, a git repo. its main starts right away.</p></div>
        <div className="b-sheet-body"><Field defaultValue="~/code/shop" readOnly /></div>
        <div className="b-sheet-foot"><Button kind="ghost">cancel</Button><Button kind="pri">add 2 projects</Button></div>
      </div>
    </>
  ),
  // the thread's signed looks as named variants (designer m_18626), each in its states
  variants: () => (
    <div className="g-col">
      <div className="g-row" data-testid="ds.variants.part">
        <Chip kind="part" of="paste" onClick={() => undefined} testId="ds.chip.part">▤ 1 · 42 lines</Chip>
        <Chip kind="part" of="paste" expanded state="hover" onClick={() => undefined}>▤ 2 · 8 lines</Chip>
        <Chip kind="part" of="file" title="~/code/shop/src/main.rs" onClick={() => undefined}>main.rs</Chip>
        <Chip kind="part" of="image" thumb={<span className="thumb none" aria-hidden="true">▣</span>}>image 1</Chip>
      </div>
      <div className="g-row" data-testid="ds.variants.ref">
        <span>perf landed on main · <Act kind="ref" testId="ds.act.ref">a1b2c3d</Act> · 3 files +42 −18</span>
        <Act kind="ref" state="hover">PR #482</Act><Act kind="ref" state="focus">PR #483</Act>
      </div>
      <div className="g-col" data-testid="ds.variants.lines">
        <Route kind="handoff" to="perf" role="product page speed" onClick={() => undefined} testId="ds.route.handoff" />
        <span>◷ <SchedHead open={false} onClick={() => undefined} testId="ds.sched">main scheduled dashboards · every 30m · next 16:56</SchedHead></span>
        <span>◷ <SchedHead>main scheduled nightly · every day 02:00</SchedHead></span>
        <BriefLine open={false} onClick={() => undefined} testId="ds.brief">bise started gift-ui · make the gift card page load under a second</BriefLine>
        <BriefLine open onClick={() => undefined}>bise started perf · product page speed</BriefLine>
        <CardLine onClick={() => undefined} testId="ds.cardline">bise asks: which branch first? · in the inbox below</CardLine>
        <CardLine onClick={() => undefined}>bise asks: keep the webp in <ArtName onClick={() => undefined}>product page, before / after</ArtName>? · in the inbox below</CardLine>
        <CardLine onClick={() => undefined}>waits for you · in the inbox below</CardLine>
      </div>
    </div>
  ),
  // an answer in the TUI's markdown colors (ambient-lead m_11457): the thread's real renderer on the
  // left, the TUI's markdown.rs rules drawn as the terminal draws them on the right
  markdown: () => (
    <div className="g-md">
      <div className="b-text"><Markdown text={MD_SAMPLE} /></div>
      <TuiMd />
    </div>
  ),
  // the bims' UI parts (designer m_37940), the app's own components: the AGENTS head's toggle (off,
  // on with its '320 · wide', cube-only at a tight head), the map chip (rest, open, the fade after a
  // pick: the old office over the new one, mid-way), '⤢ fit', the '?' / '✗' pills, a bim's labels
  bims: () => {
    const noop = () => undefined;
    const head = (k: string, w: number, on: boolean, title: string) => (
      <div className="g-agents" style={{ width: w }} data-testid={"ds.bims.toggle." + k}>
        <span className="g-agents-title"><span className="g-agents-label">{title}</span></span>
        <BimsSwitch view={{ view: on ? "bims" : "list", wide: false }} />
        <BimsToggle view={{ view: on ? "bims" : "list", wide: false }} />
        <span className="g-agents-all">all ›</span>
      </div>
    );
    return (
      <div className="g-col">
        <div className="g-row g-top">
          <Entry name="toggle off">{head("off", 304, false, "AGENTS · 3")}</Entry>
          <Entry name="toggle on">{head("on", 304, true, "AGENTS · 3")}</Entry>
          <Entry name="toggle cube-only (the 320 bar, a long count)">{head("cube-only", 304, true, "AGENTS · 128")}</Entry>
        </div>
        <div className="g-row g-top">
          <Entry name="chip rest">
            <div className="g-office" data-testid="ds.bims.map.rest"><div className="bims-mapsel"><MapChip map="loft" onPick={noop} /></div></div>
          </Entry>
          <Entry name="chip crossfade">
            <div className="g-office" data-testid="ds.bims.map.xfade">
              <div className="bims-xfade g-xfade" data-on="" />
              <div className="bims-mapsel"><MapChip map="greenhouse" onPick={noop} /></div>
            </div>
          </Entry>
          <Entry name="fit rest">
            <div className="g-office" data-testid="ds.bims.fit.rest"><FitChip onFit={noop} /></div>
          </Entry>
          <Entry name="fit hover">
            <div className="g-office g-hover" data-testid="ds.bims.fit.hover"><FitChip onFit={noop} /></div>
          </Entry>
        </div>
        <div className="g-row g-top">
          <Entry name="chip open (13 maps, capped: the fade below)">
            <div className="g-office g-office-open" data-testid="ds.bims.map.open">
              <Picker groups={[{ id: "maps", rows: mapRows(MAP_IDS, "loft") }]} maxHeight={260} onPick={noop} />
              <div className="bims-mapsel"><PickerTrigger extra="bims-chip" open>{mapName("loft")}</PickerTrigger></div>
            </div>
          </Entry>
          <div className="g-col">
            <Entry name="pills"><div className="g-row"><Pill name="search" pill="asks" onOpen={noop} /><Pill name="index" pill="failed" onOpen={noop} /></div></Entry>
            <Entry name="labels: walking, an event, with its pill">
              <div className="g-row">
                <NameLabel name="alerts" mode="name">alerts</NameLabel>
                <NameLabel name="search" mode="event">search · runs the tests</NameLabel>
                <span className="g-row g-tight"><NameLabel name="index" mode="event">index · failed</NameLabel><Pill name="index" pill="failed" onOpen={noop} /></span>
              </div>
            </Entry>
          </div>
        </div>
      </div>
    );
  },
  // bise fait mac's shared kit (page native-mac §6, mockup 6; nm-ds): the lead, the filters and the
  // bar's search, a group label, the 52 rows (rest, chosen, hovered, with an action and the '…'),
  // the empty state, and the line icons at 18, 16 and 14
  mac: () => {
    const noop = () => undefined;
    const m: EntityMenu = { open: noop, onContextMenu: noop };
    const q = <span className="g-mac-q">?</span>;
    return (
      <div className="g-mac" data-testid="ds.mac">
        {/* the banner (designer m_42762) with no vignette: the page's line icon at 64 */}
        <PageBanner title="Inbox" sentence="what your agents wait on you for, from every project." facts={<><BannerWait>3 waiting</BannerWait> · the oldest 21 d ago</>}
          icon="inbox" edge="page" testId="ds.mac.lead" />
        <ListBar inset end={<ListSearch value="" testId="ds.mac.search" />}>
          <Filters items={[{ id: "w", label: "waiting" }, { id: "a", label: "answered" }, { id: "x", label: "withdrawn" }]} on="w" onPick={noop} testId="ds.mac.filters" />
          <PickerTrigger look="pop" onClick={noop}>all projects</PickerTrigger>
        </ListBar>
        <GroupLabel count={3} waits tip="3 waiting for you" testId="ds.mac.group">waiting for you</GroupLabel>
        <ListRow on state={q} title={<><b>perf</b> run rm -rf target?</>} sub="acme · 21 d · #10" end={<><RowAction>answer</RowAction><MoreButton menu={m} /></>} testId="ds.mac.row.on" />
        <ListRow state={q} title={<><b>perf</b> which bench: cold or warm?</>} sub="acme · 21 d · #9" end={<MoreButton menu={m} />} testId="ds.mac.row" />
        <div className="g-hover"><ListRow state={q} title={<><b>bise</b> let questions reach you as macOS notifications?</>} sub="harness · now" end={<MoreButton menu={m} />} testId="ds.mac.row.hover" /></div>
        <GroupLabel testId="ds.mac.group.earlier">earlier</GroupLabel>
        <ListRow state={<AppIcon name="check" size={14} />} title={<><b>perf</b> webp or avif for the hero?</>} sub="you: avif · acme · #11" testId="ds.mac.row.done" />
        <div className="g-mac-gap" /><Empty title="nothing waits on you." testId="ds.mac.empty">your agents ask here when they need you.</Empty>
        {([18, 16, 14] as const).map((size) => (
          <div key={size} className="g-row g-mac-icons" data-testid={"ds.mac.icons." + String(size)}>
            {APP_ICONS.map((n) => <span key={n} data-tip={n}><AppIcon name={n} size={size} /></span>)}
          </div>
        ))}
      </div>
    );
  },
  window: () => (
    <Win
      side={<Side foot="+ add a project"><Row kind="me" name="bise" sub="every project" /><SectionHead>projects</SectionHead><Row kind="pj" name="telemetry" dot="nd" end="2" needs on /></Side>}
      main={<Main><Head title="telemetry" sub="main · 3 agents" dot="nd" /><Thread><Msg from="bise" name="bise" time="10:41">i asked alerts to bench 8 workers.</Msg></Thread>
        <ComposerFrame send="off" tab="you → telemetry · main"><ComposerStill placeholder="ask telemetry" /></ComposerFrame></Main>}
    />
  ),
};

export const WIDE = new Set(["mac", "presence", "markdown", "thread", "composer", "dock", "board", "sideview", "settings", "onboarding", "window", "head", "rows"]);

/** the gallery: each section, dark | light */
export function Gallery({ only }: { only?: readonly string[] }) {
  const names = Object.keys(SECTIONS).filter((n) => !only || only.length === 0 || only.includes(n));
  return (
    <div className="g-page b-ui">
      {names.map((n) => (
        <section key={n} className={WIDE.has(n) ? "g-sec wide" : "g-sec"} data-testid={"ds.gallery." + n}>
          <h2 className="g-h">{n}</h2>
          <div className="g-themes">
            {(["dark", "light"] as const).map((t) => (
              <div key={t} className="g-theme b-ui" data-theme={t} data-testid={"ds.gallery." + n + "." + t}>{SECTIONS[n]?.()}</div>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
