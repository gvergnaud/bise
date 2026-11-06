// thread-loading: a thread's newest page on its way (store.threadSyncing), the list's cue.
import { describe, expect, it } from "vitest";
import type { Entry } from "@/proto";
import type { CoreCmd } from "@/shared/api";
import { Store, threadKey } from "./store";

const P = "acme-1a2b3c4d";
const K = threadKey(P, "perf");
const entry = (pos: number, kind: Entry["kind"] = "agent"): Entry => ({ pos, at_ms: pos, kind, text: `e${String(pos)}` });
const page = (pos: number[], before: number | null = null, more = false) =>
  ({ ev: "thread", project: P, agent: "perf", entries: pos.map((p) => entry(p)), before, more }) as const;

function fake(): { sent: CoreCmd[]; store: Store; told: () => number } {
  const sent: CoreCmd[] = [];
  const store = new Store({ send: (c) => sent.push(c) }, { now: () => 1000 });
  store.setCore("up");
  let n = 0;
  store.subscribe(`sync:${K}`, () => (n += 1));
  return { sent, store, told: () => n };
}

describe("a thread's newest page on its way (thread-loading)", () => {
  it("is on from the subscribe to its page, even with old entries held", () => {
    const { store, told } = fake();
    store.subscribeThread(P, "perf");
    store.apply(page([1, 2]));
    expect(store.threadSyncing(K)).toBe(false);
    // he leaves: the entries stay, no longer live
    store.unsubscribeThread(P, "perf");
    expect(store.threadOrder(K).order).toEqual([1, 2]);
    // he comes back: the old entries show, the newest page is on its way
    store.subscribeThread(P, "perf");
    expect(store.threadSyncing(K)).toBe(true);
    store.apply(page([2, 3, 4]));
    expect(store.threadSyncing(K)).toBe(false);
    expect(store.threadOrder(K).order).toEqual([1, 2, 3, 4]);
    expect(told()).toBe(4);
  });

  it("goes when his page holds nothing newer (no change but the cue)", () => {
    const { store } = fake();
    store.subscribeThread(P, "perf");
    store.apply(page([1, 2]));
    store.unsubscribeThread(P, "perf");
    store.subscribeThread(P, "perf");
    store.apply(page([1, 2]));
    expect(store.threadSyncing(K)).toBe(false);
    expect(store.threadOrder(K).order).toEqual([1, 2]);
  });

  it("goes when he leaves before the page, a refused subscribe, or the core going down", () => {
    const { store } = fake();
    store.subscribeThread(P, "perf");
    store.unsubscribeThread(P, "perf");
    expect(store.threadSyncing(K)).toBe(false);
    store.subscribeThread(P, "perf");
    store.apply({ ev: "error", project: P, cmd: "subscribe", text: "no agent perf" });
    expect(store.threadSyncing(K)).toBe(false);
    store.subscribeThread(P, "perf");
    store.setCore("down");
    expect(store.threadSyncing(K)).toBe(false);
    // a core back up: asked again, on again until its page
    store.setCore("up");
    expect(store.threadSyncing(K)).toBe(true);
    store.apply(page([1]));
    expect(store.threadSyncing(K)).toBe(false);
  });

  it("a live entry alone doesn't end it: only the page says the view is whole", () => {
    const { store } = fake();
    store.subscribeThread(P, "perf");
    store.apply({ ev: "entry", project: P, agent: "perf", entry: entry(9) });
    expect(store.threadSyncing(K)).toBe(true);
  });
});

// boot-speed: the core's copy of main's last page, shown while the hub boots, replaced whole
describe("the core's copy of a thread's last page (boot-speed)", () => {
  const copy = (pos: number[]) => ({ ...page(pos), cached: true }) as const;

  it("shows at once with the cue on, and the hub's page replaces it whole", () => {
    const { store } = fake();
    store.subscribeThread(P, "perf");
    store.apply(copy([5, 6, 7]));
    expect(store.threadOrder(K).order).toEqual([5, 6, 7]);
    expect(store.threadSyncing(K)).toBe(true);
    // a live entry never merges into the copy: the hub's page holds it
    store.apply({ ev: "entry", project: P, agent: "perf", entry: entry(8) });
    expect(store.threadOrder(K).order).toEqual([5, 6, 7]);
    // the hub's page: only its entries, none of the copy's left
    store.apply(page([7, 8, 9]));
    expect(store.threadOrder(K).order).toEqual([7, 8, 9]);
    expect(store.threadSyncing(K)).toBe(false);
    store.apply({ ev: "entry", project: P, agent: "perf", entry: entry(10) });
    expect(store.threadOrder(K).order).toEqual([7, 8, 9, 10]);
  });

  it("never replaces what the window holds, and never steps a send", () => {
    const { store } = fake();
    store.subscribeThread(P, "perf");
    store.apply(page([1, 2]));
    store.apply(copy([5, 6]));
    expect(store.threadOrder(K).order).toEqual([1, 2]);
    // his message in a copy is not a landed send
    const { store: s2 } = fake();
    s2.subscribeThread(P, "perf");
    s2.apply({ ...copy([3]), entries: [entry(3, "you")] });
    expect(s2.threadOrder(K).order).toEqual([3]);
    s2.apply(page([4]));
    expect(s2.threadOrder(K).order).toEqual([4]);
  });
});
