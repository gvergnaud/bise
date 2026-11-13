// The store's voice side (dictation, sign-in, fn's talk words, the talk's still, loudness), moved out
// of store.test.ts as is.
import { describe, expect, it, vi } from "vitest";
import type { CoreCmd } from "@/shared/api";
import { DICTATE_STOP_WAIT_MS, Store } from "./store";

function fake(): { sent: CoreCmd[]; store: Store } {
  const sent: CoreCmd[] = [];
  const store = new Store({ send: (c) => sent.push(c) }, { now: () => 1000 });
  store.setCore("up");
  return { sent, store };
}

describe("store: voice", () => {
  it("I: a dictation's whole text replaces, one final ends it, an id it never started is ignored", () => {
    const { sent, store } = fake();
    const id = store.dictateStart();
    expect(sent).toEqual([{ cmd: "dictate_start", id }]);
    expect(store.dictation(id)).toEqual({
      text: "",
      final: false,
      cancelled: false,
      error: null,
    });
    store.apply({ ev: "dictation", id, text: "fix the", final: false });
    store.apply({ ev: "dictation", id, text: "fix the login", final: false });
    expect(store.dictation(id)?.text).toBe("fix the login");
    store.dictateStop(id);
    expect(sent.at(-1)).toEqual({ cmd: "dictate_stop", id });
    store.apply({
      ev: "dictation",
      id,
      text: "fix the login test",
      final: true,
    });
    expect(store.dictation(id)).toEqual({
      text: "fix the login test",
      final: true,
      cancelled: false,
      error: null,
    });
    // over: a late event changes nothing, stop/cancel send nothing
    store.apply({ ev: "dictation", id, text: "late", final: false });
    expect(store.dictation(id)?.text).toBe("fix the login test");
    const n = sent.length;
    store.dictateCancel(id);
    expect(sent.length).toBe(n);
    // another window's dictation never lands here
    store.apply({
      ev: "dictation",
      id: "not-mine",
      text: "secret",
      final: false,
    });
    expect(store.dictation("not-mine")).toBeNull();
    // a refused start is its final, with the error
    const busy = store.dictateStart();
    store.apply({
      ev: "dictation",
      id: busy,
      text: "",
      final: true,
      error: "the mic is busy.",
    });
    expect(store.dictation(busy)).toEqual({
      text: "",
      final: true,
      cancelled: false,
      error: "the mic is busy.",
    });
  });
  it("I: a take the core kept (no words, an error) is transcribed again on retry, dropped on drop; never one it didn't keep (amb-core 276074b2)", () => {
    const { sent, store } = fake();
    const a = store.dictateStart();
    store.dictateRetry(a);
    expect(sent.at(-1), "not over yet: no retry").toEqual({ cmd: "dictate_start", id: a });
    store.apply({ ev: "dictation", id: a, text: "", final: true, error: "the transcription timed out.", kept: true });
    expect(store.dictation(a)?.kept).toBe(true);
    store.dictateRetry(a);
    expect(sent.at(-1)).toEqual({ cmd: "dictate_retry", id: a });
    expect(store.dictation(a)).toEqual({ text: "", final: false, cancelled: false, error: null });
    store.apply({ ev: "dictation", id: a, text: "there", final: true });
    expect(store.dictation(a)).toEqual({ text: "there", final: true, cancelled: false, error: null });
    const b = store.dictateStart();
    store.apply({ ev: "dictation", id: b, text: "", final: true, kept: true });
    store.dictateDrop(b);
    expect(sent.at(-1)).toEqual({ cmd: "dictate_cancel", id: b });
    expect(store.dictation(b)?.cancelled).toBe(true);
    const n = sent.length;
    store.dictateDrop(b);
    store.dictateRetry(b);
    expect(sent.length, "a dropped take is gone").toBe(n);
  });
  it("dictation-keep: a take a draft holds that this window never saw (a relaunch) is asked again; done lets the core drop its sound", () => {
    const { sent, store } = fake();
    store.dictateRetry("d7-1");
    expect(sent.at(-1)).toEqual({ cmd: "dictate_retry", id: "d7-1" });
    expect(store.dictation("d7-1")).toEqual({ text: "", final: false, cancelled: false, error: null });
    const n = sent.length;
    store.dictateDone("d7-1");
    expect(sent.length, "never while it is transcribed").toBe(n);
    store.apply({ ev: "dictation", id: "d7-1", text: "back", final: true });
    store.dictateDone("d7-1");
    expect(sent.at(-1)).toEqual({ cmd: "dictate_cancel", id: "d7-1" });
    expect(store.dictation("d7-1")?.text, "its words stay").toBe("back");
  });
  it("I: esc ends a dictation at once, a stop the core never answers ends after a wait (amb-tools on the cut, m_20100)", () => {
    vi.useFakeTimers();
    try {
      const { sent, store } = fake();
      // a core that never answers (its mic stuck opening): esc still ends it, here, at once
      const a = store.dictateStart();
      store.dictateCancel(a);
      expect(sent.at(-1)).toEqual({ cmd: "dictate_cancel", id: a });
      expect(store.dictation(a)).toEqual({
        text: "",
        final: true,
        cancelled: true,
        error: null,
      });
      store.apply({
        ev: "dictation",
        id: a,
        text: "late",
        final: true,
        cancelled: true,
      });
      expect(
        store.dictation(a)?.text,
        "the core's late answer changes nothing",
      ).toBe("");
      // a stop with no final: the words so far stay, with a note, after the wait
      const b = store.dictateStart();
      store.apply({ ev: "dictation", id: b, text: "fix the", final: false });
      store.dictateStop(b);
      vi.advanceTimersByTime(DICTATE_STOP_WAIT_MS - 1);
      expect(store.dictation(b)?.final).toBe(false);
      vi.advanceTimersByTime(1);
      expect(store.dictation(b)).toEqual({
        text: "fix the",
        final: true,
        cancelled: false,
        error: "the mic didn't answer.",
      });
      // a long take (dictate-nocap): each segment heard after the stop starts the wait again,
      // so the window waits as long as the core still says something
      const long = store.dictateStart();
      store.dictateStop(long);
      for (let i = 1; i <= 4; i++) {
        vi.advanceTimersByTime(DICTATE_STOP_WAIT_MS - 1);
        store.apply({ ev: "dictation", id: long, text: `w${String(i)}`, final: false });
      }
      expect(store.dictation(long)).toEqual({ text: "w4", final: false, cancelled: false, error: null });
      store.apply({ ev: "dictation", id: long, text: "w1 w5", final: true, stopped: "silence" });
      vi.advanceTimersByTime(DICTATE_STOP_WAIT_MS);
      expect(store.dictation(long)).toEqual({ text: "w1 w5", final: true, cancelled: false, error: null, stopped: "silence" });
      // a stop the core answers in time: its final, the wait does nothing after
      const c = store.dictateStart();
      store.dictateStop(c);
      store.apply({ ev: "dictation", id: c, text: "on time", final: true });
      vi.advanceTimersByTime(DICTATE_STOP_WAIT_MS);
      expect(store.dictation(c)).toEqual({
        text: "on time",
        final: true,
        cancelled: false,
        error: null,
      });
    } finally {
      vi.useRealTimers();
    }
  });
  it("V14: signing holds the open sign-in link until accounts or its error", () => {
    const { store } = fake();
    expect(store.signing()).toBeNull();
    store.apply({
      ev: "signing",
      id: "chatgpt",
      url: "https://auth.openai.test/x",
    });
    expect(store.signing()).toEqual({
      id: "chatgpt",
      url: "https://auth.openai.test/x",
    });
    store.apply({ ev: "error", cmd: "sign_in", text: "chatgpt: cancelled" });
    expect(store.signing()).toBeNull();
    store.apply({
      ev: "signing",
      id: "chatgpt",
      url: "https://auth.openai.test/y",
    });
    store.apply({ ev: "accounts", items: [] });
    expect(store.signing()).toBeNull();
  });

  it("heard: fn's talk words so far, then final; a new talk starts empty (the capsule's talk view)", () => {
    const { sent, store } = fake();
    expect(store.heard()).toBeNull();
    store.apply({ ev: "heard", text: "why is the cold", final: false });
    expect(store.heard()).toEqual({ text: "why is the cold", final: false });
    store.apply({
      ev: "heard",
      text: "why is the cold bench slow",
      final: true,
    });
    expect(store.heard()).toEqual({
      text: "why is the cold bench slow",
      final: true,
    });
    store.command({ cmd: "talk_start" });
    expect(sent.at(-1)).toEqual({ cmd: "talk_start" });
    expect(store.heard()).toBeNull();
  });

  it("shot: the talk's still while it is stored, null once it goes; a new talk starts with none (ambient-lead m_21912)", () => {
    const { sent, store } = fake();
    expect(store.shot()).toBeNull();
    store.apply({ ev: "shot", file: "/x/.bise/images/9f.png", app: "Safari", title: "cold bench · grafana" });
    expect(store.shot()).toEqual({ file: "/x/.bise/images/9f.png", id: null, app: "Safari", title: "cold bench · grafana" });
    store.apply({ ev: "shot", file: null });
    expect(store.shot()).toBeNull();
    store.apply({ ev: "shot", file: "/x/.bise/images/a0.png", id: 3 });
    expect(store.shot()).toEqual({ file: "/x/.bise/images/a0.png", id: 3, app: null, title: null });
    store.command({ cmd: "shot_drop" });
    expect(sent.at(-1)).toEqual({ cmd: "shot_drop" });
    store.command({ cmd: "shot_drop", id: 3 });
    expect(sent.at(-1)).toEqual({ cmd: "shot_drop", id: 3 });
    store.command({ cmd: "talk_start" });
    expect(store.shot()).toBeNull();
  });

  it("spot-shot: the spotlight's ask goes via the capsule and names the still it showed; a plain send names none", () => {
    const { sent, store } = fake();
    store.send({ project: "home", agent: "main", text: "why red?", via: "capsule", shot: 3 });
    expect(sent.at(-1)).toMatchObject({ cmd: "send", project: "home", agent: "main", text: "why red?", via: "capsule", shot: 3 });
    store.send({ project: "home", agent: "main", text: "and this?" });
    const plain = sent.at(-1) as Record<string, unknown>;
    expect("via" in plain || "shot" in plain).toBe(false);
  });

  it("level: each voice's loudness, the last value, a repeat changes nothing", () => {
    const { store } = fake();
    expect(store.level("you")).toBe(0);
    let n = 0;
    store.subscribe("level:you", () => (n += 1));
    store.apply({ ev: "level", who: "you", v: 0.42 });
    store.apply({ ev: "level", who: "you", v: 0.42 });
    store.apply({ ev: "level", who: "main", v: 0.9 });
    expect([store.level("you"), store.level("main"), n]).toEqual([
      0.42, 0.9, 1,
    ]);
  });
});
