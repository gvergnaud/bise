import { describe, expect, it } from "vitest";
import { app_cmd, core_ev, hub_cmd, hub_ev } from "@/proto/fixtures.gen";
import type { Entry, Part } from "@/proto";
import type { CoreCmd } from "@/shared/api";
import { allFixtureEvents, replay } from "./replay";
import { sentWords } from "./types";
import { OLDER_LIMIT } from "./window";
import {
  MARKS,
  Store,
  TAGS,
  threadKey,
  typed,
} from "./store";

const P = "acme-1a2b3c4d";
const K = threadKey(P, "perf");

function fake(): { sent: CoreCmd[]; store: Store } {
  const sent: CoreCmd[] = [];
  const store = new Store({ send: (c) => sent.push(c) }, { now: () => 1000 });
  store.setCore("up");
  return { sent, store };
}

const entry = (
  pos: number,
  text: string,
  kind: Entry["kind"] = "agent",
): Entry => ({ pos, at_ms: pos, kind, text });

describe("store", () => {
  it("drops the hub's plain line at its next hello, keeps a refused one (his m_22835)", () => {
    const { store } = fake();
    const hello = { ev: "welcome", project: P, proto: 1, workspace: "/w", name: "acme" } as const;
    store.apply({ ev: "notice", project: P, cmd: "slash", text: "restarting the hub on 041168b2 · the agents keep running", cid: 7 });
    expect(store.notice(P)?.level).toBe("note");
    store.apply(hello);
    expect(store.notice(P)).toBeNull();
    store.apply({ ev: "error", project: P, cmd: "slash", text: "/close 999: no such item", cid: 8, reason: "refused" });
    store.apply(hello);
    expect(store.notice(P)).toEqual({ text: "/close 999: no such item", level: "error" });
  });

  it("keeps the approvals mode and rules per project, whole each time (V8/W21)", () => {
    const { store } = fake();
    expect(store.approvals(P)).toBeNull();
    const rule = {
      tool: "bash",
      pattern: "cargo test *",
      added: "1791100000000",
      added_ms: 1791100000000,
      what: "cargo test *",
      note: "from perf",
    };
    store.apply({
      ev: "approvals",
      project: P,
      mode: "auto",
      env: false,
      checker: "jev",
      checker_who: "TypeSafe",
      repo: "/w",
      rules: [rule],
      flash: true,
    });
    expect(store.approvals(P)?.mode).toBe("auto");
    expect(store.approvals(P)?.rules.map((r) => r.what)).toEqual([
      "cargo test *",
    ]);
    store.apply({
      ev: "approvals",
      project: P,
      mode: "yolo",
      env: false,
      checker: "jev",
      checker_who: "TypeSafe",
      repo: "/w",
      rules: [],
    });
    expect(store.approvals(P)?.mode).toBe("yolo");
    expect(store.approvals(P)?.rules).toEqual([]);
    expect(store.approvals("other")).toBeNull();
  });

  it("keeps the live scheduled tasks per project, whole each time", () => {
    const { store } = fake();
    expect(store.scheduled(P)).toEqual([]);
    const task = {
      id: 3,
      agent: "main",
      by: "main",
      words: "every 10m",
      every: "10m",
      done: 0,
      next_ms: 1791100600000,
    };
    store.apply({ ev: "scheduled", project: P, items: [task] });
    expect(store.scheduled(P).map((t) => t.id)).toEqual([3]);
    store.apply({ ev: "scheduled", project: P, items: [] });
    expect(store.scheduled(P)).toEqual([]);
    expect(store.scheduled("other")).toEqual([]);
  });

  it("takes every fixture event and skips unknown tags", () => {
    const { store } = fake();
    const events = allFixtureEvents();
    expect(replay(store, events)).toBe(events.length);
    expect(
      store.apply({ ev: "spotlight_hint", project: P, text: "a newer hub" }),
    ).toBe(false);
    expect(store.apply({ cmd: "approve" })).toBe(false);
    expect(store.apply("not an object")).toBe(false);
    expect(store.apply({ ev: "orb", phase: "idle" })).toBe(false);
  });

  it("knows every tag of the fixtures (TAGS is the generated unions)", () => {
    for (const ev of [...core_ev, ...hub_ev])
      expect(Object.hasOwn(TAGS, ev.ev)).toBe(true);
    expect(typed({ ev: "welcome" })).not.toBeNull();
  });

  it("merges thread pages and keeps the newest page before/more after an older page", () => {
    const { store } = fake();
    replay(
      store,
      hub_ev.filter((e) => e.ev === "thread"),
    );
    const v = store.threadOrder(K);
    expect(v.loaded).toBe(true);
    expect([...v.order]).toEqual([...v.order].sort((a, b) => a - b));
    // a later page of older history (pos 2, the hub's order) lands in its place, not at the end
    expect(v.order.slice(0, 3)).toEqual([1, 2, 3]);
    expect(v.order.at(-1)).not.toBe(2);
  });

  it("an entry replaces only that entry: other entries keep their object", () => {
    const { store } = fake();
    store.apply({
      ev: "thread",
      project: P,
      agent: "perf",
      entries: [entry(1, "a"), entry(2, "b")],
      before: null,
      more: false,
    });
    const one = store.entry(K, 1);
    const order = store.threadOrder(K);
    const seen: string[] = [];
    store.subscribe(`entry:${K}:1`, () => seen.push("1"));
    store.subscribe(`entry:${K}:2`, () => seen.push("2"));
    store.subscribe(`order:${K}`, () => seen.push("order"));
    store.apply({
      ev: "entry",
      project: P,
      agent: "perf",
      entry: entry(2, "b, longer"),
    });
    expect(seen).toEqual(["2"]);
    expect(store.entry(K, 1)).toBe(one);
    expect(store.threadOrder(K)).toBe(order);
    store.apply({
      ev: "entry",
      project: P,
      agent: "perf",
      entry: entry(3, "c"),
    });
    expect(seen).toEqual(["2", "order"]);
    expect(store.threadOrder(K).order).toEqual([1, 2, 3]);
    // the same entry again changes nothing
    store.apply({
      ev: "entry",
      project: P,
      agent: "perf",
      entry: entry(3, "c"),
    });
    expect(seen).toEqual(["2", "order"]);
  });

  it("a steer's receipt (an entry re-sent with steered_at) tells the order's readers: the rows move", () => {
    const { store } = fake();
    store.apply({
      ev: "thread",
      project: P,
      agent: "perf",
      entries: [entry(1, "a", "you"), entry(2, "b", "tools"), entry(3, "c")],
      before: null,
      more: false,
    });
    const order = store.threadOrder(K);
    const seen: string[] = [];
    store.subscribe(`order:${K}`, () => seen.push("order"));
    store.apply({
      ev: "entry",
      project: P,
      agent: "perf",
      entry: { ...entry(1, "a", "you"), steered_at: 2 },
    });
    expect(seen).toEqual(["order"]);
    expect(store.threadOrder(K)).not.toBe(order);
    expect(store.threadOrder(K).order).toEqual([1, 2, 3]);
    expect(store.entry(K, 1)?.steered_at).toBe(2);
    // the same mark again moves nothing
    store.apply({
      ev: "entry",
      project: P,
      agent: "perf",
      entry: { ...entry(1, "a", "you"), steered_at: 2 },
    });
    expect(seen).toEqual(["order"]);
  });

  it("typing re-renders only typing, and only on a change", () => {
    const { store } = fake();
    const seen: string[] = [];
    store.subscribe(`typing:${K}`, () => seen.push("typing"));
    store.subscribe(`order:${K}`, () => seen.push("order"));
    store.apply({
      ev: "typing",
      project: P,
      agent: "perf",
      text: "running the tests",
    });
    store.apply({
      ev: "typing",
      project: P,
      agent: "perf",
      text: "running the tests",
    });
    expect(seen).toEqual(["typing"]);
    expect(store.typing(K)).toBe("running the tests");
  });

  it("a send shows at once, is sent typed, and goes when its you entry lands (FIFO)", () => {
    const { store, sent } = fake();
    store.send({ project: P, agent: "perf", text: "again" });
    store.send({ project: P, agent: "perf", text: "again" });
    expect(store.pending(K).map((p) => p.state)).toEqual([
      "sending",
      "sending",
    ]);
    expect(sent).toEqual([
      {
        cmd: "send",
        project: P,
        agent: "perf",
        text: "again",
        mode: "now",
        cid: 1,
      },
      {
        cmd: "send",
        project: P,
        agent: "perf",
        text: "again",
        mode: "now",
        cid: 2,
      },
    ]);
    const first = store.pending(K)[0]?.cid ?? 0;
    // the hub answers each send with its message id; the entry with that id takes its row
    store.apply({ ev: "sent", project: P, agent: "perf", cid: first, msg_id: 9, reason: "made" });
    store.apply({
      ev: "entry",
      project: P,
      agent: "perf",
      entry: { ...entry(9, "again", "you"), msg_id: 9 },
    });
    expect(store.pending(K).map((p) => p.cid)).not.toContain(first);
    expect(store.pending(K)).toHaveLength(1);
  });

  it("a send goes when its echo comes in a page too; an older entry with the same words never takes it (ambient-lead m_20138)", () => {
    const sent: CoreCmd[] = [];
    let now = 100_000;
    const store = new Store({ send: (c) => sent.push(c) }, { now: () => now });
    store.setCore("up");
    const you = (pos: number, at_ms: number, text: string, more: Partial<Entry> = {}): Entry => ({ pos, at_ms, kind: "you", text, ...more });
    const ok = store.send({ project: P, agent: "perf", text: "ok" });
    store.apply({ ev: "sent", project: P, agent: "perf", cid: ok, msg_id: 12, reason: "made" });
    // the thread opened after the send: a page with an old 'ok' only, then one with his echo
    store.apply({ ev: "thread", project: P, agent: "perf", entries: [you(3, 20_000, "ok")], before: null, more: false });
    expect(store.pending(K).map((p) => p.text)).toEqual(["ok"]);
    // the old one sent again with its steered_at (an entry event) still is not his new one
    store.apply({ ev: "entry", project: P, agent: "perf", entry: you(3, 20_000, "ok", { steered_at: 4, msg_id: 7, delivery: "read" }) });
    expect(store.pending(K)).toHaveLength(1);
    store.apply({ ev: "thread", project: P, agent: "perf", entries: [you(3, 20_000, "ok"), you(9, 100_400, "ok", { msg_id: 12 })], before: null, more: false });
    expect(store.pending(K)).toEqual([]);
    // a steered one: its echo is an entry, re-sent with its marks (received, then read + steered_at)
    now = 200_000;
    const webp = store.send({ project: P, agent: "perf", text: "also try webp" });
    store.apply({ ev: "sent", project: P, agent: "perf", cid: webp, msg_id: 13, reason: "made" });
    store.apply({ ev: "entry", project: P, agent: "perf", entry: you(11, 200_100, "also try webp", { msg_id: 13, delivery: "sent" }) });
    expect(store.pending(K)).toEqual([]);
    store.apply({ ev: "entry", project: P, agent: "perf", entry: you(11, 200_100, "also try webp", { msg_id: 13, delivery: "read", steered_at: 12 }) });
    expect(store.pending(K)).toEqual([]);
    // a queued one: its echo is written when it is said, later, and still takes it
    now = 300_000;
    const log = store.send({ project: P, agent: "perf", text: "and the changelog", mode: "queued" });
    store.apply({ ev: "sent", project: P, agent: "perf", cid: log, msg_id: 14, reason: "made" });
    store.apply({ ev: "entry", project: P, agent: "perf", entry: you(20, 360_000, "and the changelog", { msg_id: 14 }) });
    expect(store.pending(K)).toEqual([]);
  });

  it("a send the core went down before answering goes when its echo lands with the hub's marker before his words (qa-native R41, send-reason)", () => {
    const { store } = fake();
    store.setCore("up");
    store.send({ project: P, agent: "perf", text: "what is in this picture?" });
    store.send({ project: P, agent: "perf", text: "[Image #1] and this one" });
    expect(store.pending(K)).toHaveLength(2);
    // the core died before their answers: lost, only their words and parts can meet their echo
    store.setCore("down");
    store.setCore("up");
    const marker =
      '<image name="[Image #1]" path="/tmp/shot.png" mime="image/png" b64="/h/images/ab12.b64">';
    store.apply({
      ev: "entry",
      project: P,
      agent: "perf",
      entry: entry(
        9,
        `${marker}
what is in this picture?`,
        "you",
      ),
    });
    expect(store.pending(K).map((p) => p.text)).toEqual([
      "[Image #1] and this one",
    ]);
    store.apply({
      ev: "entry",
      project: P,
      agent: "perf",
      entry: entry(10, `${marker.replace("shot", "b")} and this one`, "you"),
    });
    expect(store.pending(K)).toEqual([]);
    expect(
      sentWords(
        '<image name="[Image #2]" path="a b.png" mime="image/png" b64="x">  hi\n there',
      ),
    ).toBe("hi there");
    // file-chips: a file's link (missing or not) and its '[File #n]' label go the same way
    expect(
      sentWords(
        'add <image name="[Image #1]" path="/u/a.png" mime="image/png" b64=""> to [deck q3.pptx](/u/deck q3.pptx) and [q3.pdf](</u/q3 (1).pdf>) (missing) now',
      ),
    ).toBe("add to and now");
    expect(sentWords("add [Image #1] to [File #2] and [File #3]")).toBe(
      "add to and",
    );
  });

  it("a send carries its parts, and a retry sends them again; no parts means no field", () => {
    const { store, sent } = fake();
    const parts: Part[] = [
      { kind: "text", text: "what stands out here " },
      { kind: "file", name: "[File #1]", path: "/Users/ana/notes/q3-notes.md" },
    ];
    const cid = store.send({
      project: P,
      agent: "main",
      text: "what stands out here [File #1]",
      parts,
    });
    store.send({ project: P, agent: "main", text: "plain", parts: [] });
    expect(sent).toEqual([
      {
        cmd: "send",
        project: P,
        agent: "main",
        text: "",
        mode: "now",
        parts,
        cid: 1,
      },
      {
        cmd: "send",
        project: P,
        agent: "main",
        text: "plain",
        mode: "now",
        cid: 2,
      },
    ]);
    store.apply({ ev: "error", project: P, cmd: "send", text: "hub gone" });
    store.retry(threadKey(P, "main"), cid);
    expect(sent[2]).toEqual({
      cmd: "send",
      project: P,
      agent: "main",
      text: "",
      mode: "now",
      parts,
      cid: 3,
    });
  });

  it("G: a send's error with its cid fails exactly that row; undelivered: the thread's entry stands in for it", () => {
    const { store, sent } = fake();
    const one = store.send({ project: P, agent: "perf", text: "one" });
    const two = store.send({ project: P, agent: "perf", text: "two" });
    const three = store.send({ project: P, agent: "perf", text: "three" });
    expect(sent.map((c) => (c as { cid?: number }).cid)).toEqual([
      one,
      two,
      three,
    ]);
    // refused: only the second row, with the words; the oldest stays sending
    store.apply({
      ev: "error",
      project: P,
      cmd: "send",
      text: "no agent perf",
      cid: two,
      reason: "refused",
    });
    expect(store.pending(K).map((p) => [p.cid, p.state, p.why])).toEqual([
      [one, "sending", undefined],
      [two, "failed", "no agent perf"],
      [three, "sending", undefined],
    ]);
    // undelivered: the hub's undelivered entry is in the thread; his row goes, one fact
    store.apply({
      ev: "error",
      project: P,
      cmd: "send",
      text: "@perf is archived",
      cid: three,
      reason: "undelivered",
    });
    expect(store.pending(K).map((p) => p.cid)).toEqual([one, two]);
    // an unknown cid changes nothing
    store.apply({
      ev: "error",
      project: P,
      cmd: "send",
      text: "x",
      cid: 999,
      reason: "refused",
    });
    expect(store.pending(K).map((p) => p.state)).toEqual(["sending", "failed"]);
  });

  it("a send's error fails the oldest sending row of that project, with the hub's words", () => {
    const { store } = fake();
    store.send({ project: P, agent: "perf", text: "one" });
    store.send({ project: "other", agent: "main", text: "two" });
    store.apply({
      ev: "error",
      project: P,
      cmd: "send",
      text: "send: context isn't supported yet",
    });
    expect(store.pending(K)[0]).toMatchObject({
      state: "failed",
      why: "send: context isn't supported yet",
    });
    expect(store.pending(threadKey("other", "main"))[0]?.state).toBe("sending");
  });

  it("the core going down fails every send on its way; a send while down fails at once; retry sends again", () => {
    const { store, sent } = fake();
    store.send({ project: P, agent: "perf", text: "one" });
    store.setCore("down");
    expect(store.pending(K)[0]).toMatchObject({
      state: "failed",
      why: "not sent",
    });
    store.send({ project: P, agent: "perf", text: "two" });
    expect(store.pending(K)[1]?.state).toBe("failed");
    expect(sent).toHaveLength(1);
    store.setCore("up");
    const cid = store.pending(K)[0]?.cid ?? -1;
    store.retry(K, cid);
    expect(sent).toHaveLength(2);
    expect(store.pending(K).map((p) => [p.text, p.state])).toEqual([
      ["two", "failed"],
      ["one", "sending"],
    ]);
    store.dismiss(K, store.pending(K)[0]?.cid ?? -1);
    expect(store.pending(K).map((p) => p.text)).toEqual(["one"]);
  });

  it("cards: a project's are its projects row's, the one copy (inbox-sync); the same reference until they change", () => {
    const { store } = fake();
    replay(store, core_ev);
    replay(store, hub_ev);
    const row = store.projects().find((r) => r.project === P);
    const cards = store.cards(P);
    expect(cards).toEqual(row?.cards ?? []);
    // the same cards in a new rows event: the same array
    store.apply({ ev: "projects", projects: store.projects().map((r) => ({ ...r, cards: r.cards?.map((c) => ({ ...c })) })) });
    expect(store.cards(P)).toBe(cards);
    expect(store.agents(P)).toBe(store.agents(P));
    // the rows drop them: so does every reader, whatever the hub's last cards event said
    store.apply({ ev: "projects", projects: store.projects().map((r) => ({ ...r, cards: [] })) });
    expect(store.cards(P)).toEqual([]);
  });

  it("cards: a hub's own cards event changes nothing; the rows (his and bise's own by id, as the core lists them) do", () => {
    const { store } = fake();
    replay(store, core_ev);
    const ask = "main suggests: archive @gone? the agent is mid-turn. (answer yes or no)";
    const q = { agent: "perf", id: 3, kind: "question", options: [], project: P, question: "which bench?", rank: 1, since_ms: 1, text: "which bench?", urgent: false };
    const drop = { agent: "gone", id: 2, kind: "drop", options: [], project: P, question: ask, rank: 4, since_ms: 2, text: ask, urgent: false };
    const was = store.cards(P);
    store.apply({ ev: "cards", project: P, cards: [q], others: [drop] });
    expect(store.cards(P)).toBe(was);
    store.apply({ ev: "projects", projects: store.projects().map((r) => (r.project === P ? { ...r, cards: [drop, q] } : r)) });
    expect(store.cards(P).map((c) => [c.id, c.kind])).toEqual([
      [2, "drop"],
      [3, "question"],
    ]);
  });

  it("diffs are kept per agent and per commit; jobs, merged, worktrees per project", () => {
    const { store } = fake();
    // the terminal's /diff answers (a req) left out: the review's diffs only
    replay(
      store,
      hub_ev.filter((e) => !(e.ev === "diff" && e.req != null)),
    );
    expect(store.diff(P, "perf")?.files.length).toBeGreaterThan(0);
    expect(store.diff(P, "old")?.merged).toMatch(/^e0f3df5/);
    expect(store.jobs(P).length).toBeGreaterThan(0);
    expect(store.merged(P).length).toBeGreaterThan(0);
    expect(store.worktrees(P).length).toBeGreaterThan(0);
  });

  it("one-shot events reach onEvent; the actions send the typed commands of the fixtures", () => {
    const { store, sent } = fake();
    const ends: string[] = [];
    store.onEvent((ev) => {
      if (ev.ev === "job_end") ends.push(ev.agent);
    });
    replay(store, hub_ev);
    expect(ends).toEqual(["perf", "docs"]);
    store.subscribeThread(P, "perf", 50);
    store.do({ i: "answer", project: P, card: 9, reply: "1" });
    store.stop(P, "perf");
    for (const c of sent) expect(hub_cmd.map((h) => h.cmd)).toContain(c.cmd);
  });

  it("R41: an answer carries the images he attached as parts (as a send); none: no parts field", () => {
    const { store, sent } = fake();
    const a: Part[] = [{ kind: "image", name: "[Image #1]", source: "/Users/me/.bise/images/a.png" }];
    store.do({ i: "answer", project: P, card: 9, reply: "this one", parts: a });
    store.do({ i: "answer", project: P, card: 10, reply: "yes", parts: [] });
    store.do({ i: "answer", project: P, card: 10, reply: "yes" });
    expect(sent.filter((c) => c.cmd === "answer")).toEqual([
      { cmd: "answer", project: P, card: 9, reply: "this one", parts: a },
      { cmd: "answer", project: P, card: 10, reply: "yes" },
      { cmd: "answer", project: P, card: 10, reply: "yes" },
    ]);
  });

  it("shown is sent deduped, only when the set changes, and again to a core that comes back up", () => {
    const { store, sent } = fake();
    store.shown(["home", P, "home"]);
    store.shown(["home", P]);
    store.shown(["home"]);
    expect(sent).toEqual([
      { cmd: "shown", projects: ["home", P] },
      { cmd: "shown", projects: ["home"] },
    ]);
    store.setCore("down");
    store.setCore("starting");
    store.setCore("up");
    expect(sent[2]).toEqual({ cmd: "shown", projects: ["home"] });
    expect(app_cmd.map((c) => c.cmd)).toContain("shown");
  });

  it("a core that comes back up gets shown and every followed thread again, not the dropped ones (qa-native m_9736)", () => {
    const { store, sent } = fake();
    store.shown(["home", P]);
    store.subscribeThread(P, "perf", 60);
    store.subscribeThread(P, "main");
    store.subscribeThread(P, "old");
    store.unsubscribeThread(P, "old");
    sent.length = 0;
    store.setCore("down");
    store.setCore("starting");
    expect(sent).toEqual([]);
    store.setCore("up");
    expect(sent).toEqual([
      { cmd: "shown", projects: ["home", P] },
      { cmd: "subscribe", project: P, agent: "perf", limit: 60 },
      { cmd: "marks", project: P, agent: "perf", limit: MARKS },
      { cmd: "subscribe", project: P, agent: "main" },
      { cmd: "marks", project: P, agent: "main", limit: MARKS },
    ]);
  });

  it("a thread's subscribe asks his last messages too, and thread_marks is its marks slice (msg-dots)", () => {
    const { store, sent } = fake();
    store.subscribeThread(P, "perf");
    expect(sent.at(-1)).toEqual({ cmd: "marks", project: P, agent: "perf", limit: MARKS });
    expect(store.threadMarks(threadKey(P, "perf"))).toBeNull();
    store.apply({ ev: "thread_marks", project: P, agent: "perf", marks: [{ pos: 7, at_ms: 1, text: "go" }], upto: 90 });
    expect(store.threadMarks(threadKey(P, "perf"))).toEqual({ marks: [{ pos: 7, at_ms: 1, text: "go" }], upto: 90 });
  });

  it("olderPage asks for the page before the oldest entry, only when there is more", () => {
    const { store, sent } = fake();
    store.olderPage(P, "perf");
    expect(sent).toHaveLength(0);
    store.apply({
      ev: "thread",
      project: P,
      agent: "perf",
      entries: [entry(50, "x")],
      before: 50,
      more: true,
    });
    store.olderPage(P, "perf", 20);
    expect(sent).toEqual([
      { cmd: "page", project: P, agent: "perf", before: 50, limit: 20 },
    ]);
  });
  it("his scroll up asks OLDER_LIMIT entries a page, never the hub's default 40 (thread-older)", () => {
    // mutation: olderPage's default limit dropped (the hub's 40) turns it red
    const { store, sent } = fake();
    store.apply({ ev: "thread", project: P, agent: "main", entries: [entry(50, "x")], before: 50, more: true });
    store.olderPage(P, "main");
    expect(sent).toEqual([{ cmd: "page", project: P, agent: "main", before: 50, limit: OLDER_LIMIT }]);
    expect(OLDER_LIMIT).toBe(200);
  });
  it("C/A/B: commands asked once per core, skills per project, an @ query resolves with its newest rid only", async () => {
    const { sent, store } = fake();
    store.askCommands();
    store.askCommands();
    expect(sent.filter((c) => c.cmd === "commands")).toHaveLength(1);
    store.apply({
      ev: "commands",
      items: [
        {
          name: "/new",
          desc: "start an agent",
          args: [{ kind: "text" }],
          runs: "hub",
          runnable: true,
        },
      ],
    });
    expect(store.commands().map((c) => c.name)).toEqual(["/new"]);
    store.askSkills(P);
    store.apply({
      ev: "skills",
      project: P,
      items: [{ name: "bise-pages", desc: "a page" }],
    });
    expect(store.skills(P)).toEqual([{ name: "bise-pages", desc: "a page" }]);
    // two queries: the first is replaced (null), only the newest rid lands
    const first = store.files(P, "ma");
    const second = store.files(P, "mai");
    const rids = sent
      .filter((c) => c.cmd === "files")
      .map((c) => (c as { rid: number }).rid);
    expect(await first).toBeNull();
    store.apply({
      ev: "files",
      project: P,
      rid: rids[0] ?? 0,
      q: "ma",
      items: [{ path: "late.rs", dir: false }],
    });
    expect(store.filesNow(P)).toBeNull();
    store.apply({
      ev: "files",
      project: P,
      rid: rids[1] ?? 0,
      q: "mai",
      items: [{ path: "src/main.rs", dir: false }],
      partial: true,
    });
    expect(await second).toEqual({
      q: "mai",
      items: [{ path: "src/main.rs", dir: false }],
      folder: null,
      partial: true,
    });
    // the fuller answer of the same rid lands in the slice
    store.apply({
      ev: "files",
      project: P,
      rid: rids[1] ?? 0,
      q: "mai",
      items: [
        { path: "src/main.rs", dir: false },
        { path: "main.md", dir: false },
      ],
    });
    expect(store.filesNow(P)?.items).toHaveLength(2);
    // a core that comes up again is asked the commands again
    store.setCore("down");
    store.setCore("up");
    expect(sent.filter((c) => c.cmd === "commands")).toHaveLength(2);
  });
});
