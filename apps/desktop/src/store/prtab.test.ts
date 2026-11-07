// The store's laws for a PR tab's asks (pr/detail, pr/checkLog, pr/act, its diff): each answer is
// known by its req, only the newest ask of a key lands, any other is dropped (architect m_54668).
// Owner: pr-artifacts.
import { describe, expect, it } from "vitest";
import type { CoreCmd } from "@/shared/api";
import { Store } from "./store";

const P = "acme-1a2b3c4d";
const URL = "https://github.com/acme/web/pull/412";

function fake(): { sent: CoreCmd[]; store: Store } {
  const sent: CoreCmd[] = [];
  const store = new Store({ send: (c) => sent.push(c) }, { now: () => 1000 });
  store.setCore("up");
  return { sent, store };
}

const reqOf = (c: CoreCmd | undefined): number => {
  const r = c != null && "req" in c ? c.req : undefined;
  if (typeof r !== "number") throw new Error("no req");
  return r;
};

describe("a PR tab's asks", () => {
  it("the detail: its newest ask's answer lands, an older one's and a stranger's are dropped", () => {
    const { sent, store } = fake();
    store.askPrDetail(P, URL);
    const first = reqOf(sent.at(-1));
    store.askPrDetail(P, URL);
    const second = reqOf(sent.at(-1));
    expect(sent.at(-1)).toEqual({ cmd: "pr_detail", project: P, url: URL, req: second });
    store.apply({ ev: "pr_detail", project: P, req: first, url: URL, err: "late" });
    expect(store.prDetail(P, URL), "an older ask's: dropped").toBeNull();
    store.apply({ ev: "pr_detail", project: P, req: 999, url: URL, err: "late" });
    expect(store.prDetail(P, URL), "a req this window never sent").toBeNull();
    const detail = { body: "slow signup", checks: [], notes: [] };
    store.apply({ ev: "pr_detail", project: P, req: second, url: URL, detail });
    expect(store.prDetail(P, URL)?.detail).toEqual(detail);
    store.apply({ ev: "pr_detail", project: P, req: second, url: URL, err: "late" });
    expect(store.prDetail(P, URL)?.detail, "answered once").toEqual(detail);
  });

  it("a check's log: null while it comes, then its lines", () => {
    const { sent, store } = fake();
    store.askPrCheckLog(P, URL, "ci / test");
    expect(store.prCheckLog(P, URL, "ci / test")).toBeNull();
    const req = reqOf(sent.at(-1));
    store.apply({ ev: "pr_check_log", project: P, req, url: URL, check: "ci / test", lines: ["FAIL a"] });
    expect(store.prCheckLog(P, URL, "ci / test")?.lines).toEqual(["FAIL a"]);
    expect(store.prCheckLog(P, URL, "lint")).toBeNull();
  });

  it("his merge: on its way at the head he saw, then the hub's refusal typed", () => {
    const { sent, store } = fake();
    store.prAct(P, URL, "abc123", "merge");
    expect(store.prActed(P, URL)).toEqual({ act: "merge", done: false });
    const req = reqOf(sent.at(-1));
    expect(sent.at(-1)).toEqual({ cmd: "pr_act", project: P, url: URL, head: "abc123", act: "merge", req });
    store.apply({ ev: "pr_acted", project: P, req, url: URL, act: "merge", err: "moved" });
    expect(store.prActed(P, URL)).toEqual({ act: "merge", done: true, err: "moved" });
  });

  it("its diff: asked by link, known by its req, never on an agent's review", () => {
    const { sent, store } = fake();
    store.askPrDiff(P, URL);
    const req = reqOf(sent.at(-1));
    expect(sent.at(-1)).toEqual({ cmd: "diff", project: P, pr_url: URL, req });
    store.apply({ ev: "diff", project: P, agent: "perf", base: "main", files: [], req });
    expect(store.prDiff(P, URL)).toMatchObject({ base: "main" });
    expect(store.diff(P, "perf")).toBeNull();
  });
});
