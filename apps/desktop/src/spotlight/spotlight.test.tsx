// @vitest-environment happy-dom
// The spotlight: its list rules (the capsule's, signed by ambient), the app's composer field that
// stays one node while the list changes under it (bug 7), and its keys (Gabriel's v4): each rule
// here fails without its line in Spotlight.tsx's keyFirst.
import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Editor } from "@tiptap/core";
import { TextSelection } from "@tiptap/pm/state";
import type { Store } from "@/composer/drafts";
import { flat, spotList, spotView, type SpotData } from "./list";
import { Spotlight, answerLines, outgoingOf, quoteChip, withQuote, ANSWER_MAX_LINES } from "./Spotlight";
import { cardLine, newBrief, shortPath } from "./words";

afterEach(cleanup);

const D: SpotData = {
  projects: [
    {
      id: "tel",
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
      project: "tel",
      status: "working",
      title: "sort_unstable in the hot query",
    },
    {
      name: "alerts",
      project: "tel",
      status: "idle",
      title: "page on p99 regressions",
    },
  ],
  cards: [
    {
      id: "c1",
      card: 1,
      agent: "alerts",
      project: "tel",
      kind: "question",
      text: "Which threshold?",
    },
  ],
  front: {
    app: "Chrome",
    title: "Pricing · Acme",
    url: "https://acme.test/pricing",
  },
};

describe("the spotlight list", () => {
  it("empty: waiting for you, at work, projects with what is in front first, no ⌘ on it", () => {
    const l = spotList(D, "");
    expect(l.sections.map((s) => s.label)).toEqual([
      "waiting for you",
      "at work",
      "projects",
    ]);
    const envs = l.sections[2]?.items ?? [];
    expect(envs[0]).toMatchObject({
      kind: "front",
      name: "Pricing · Acme",
      sub: "in front: acme.test · Chrome",
    });
    expect(envs[1]).toMatchObject({
      kind: "env",
      name: "telemetry",
      n: 1,
      sub: "2 agents · main · ~/code/telemetry",
    });
  });
  it("a word names agents and projects; a sentence is a message: only the agents it names stay", () => {
    expect(flat(spotList(D, "perf")).map((i) => i.id)).toEqual([
      "agent.tel.perf",
    ]);
    expect(flat(spotList(D, "tele")).map((i) => i.id)).toEqual([
      "agent.tel.alerts",
      "agent.tel.perf",
      "env.tel",
    ]);
    const ask = spotList(D, "why is the hot query slow for perf");
    expect(ask.mode).toBe("ask");
    expect(flat(ask).map((i) => i.id)).toEqual(["agent.tel.perf"]);
    // a message with no result: no list, the field alone
    expect(spotList(D, "can you check this against the POC?").sections).toEqual([]);
  });
  it("Gabriel's v4: a line break or a chip makes a message whatever its words; a name alone is a search", () => {
    expect(spotView(D, "perf").mode).toBe("names");
    const lines = spotView(D, "perf\ncheck the sort");
    expect(lines.mode).toBe("ask");
    expect(flat(lines).map((i) => i.id)).toEqual(["agent.tel.perf"]);
    expect(spotView(D, "\uFFFC tele").mode).toBe("ask");
    expect(spotView(D, "\uFFFC").sections).toEqual([]);
    expect(spotView(D, "").mode).toBe("empty");
  });
  it("⌘K's rule (main m_12973): a sentence keeps the archived agents it names too, after the live ones", () => {
    const old = {
      name: "perf-v1",
      project: "tel",
      status: "done" as const,
      title: "slow hot query, first pass",
      archived: true,
    };
    const d: SpotData = { ...D, agents: [old, ...D.agents] };
    expect(spotList(d, "perf").mode).toBe("names");
    const ask = spotList(d, "why is the hot query slow for perf");
    expect(flat(ask).map((i) => i.id)).toEqual([
      "agent.tel.perf",
      "agent.tel.perf-v1",
    ]);
  });
  it("words: card lines and paths as ambient signed them", () => {
    expect(
      cardLine("t1-97894 is ready to try\n1 commit on t1-97894 · +3 −0"),
    ).toBe("t1-97894 is ready to try · 1 commit · +3 −0");
    expect(shortPath("/Users/me/.bise/gate/w-ambtools/h/code/telemetry")).toBe(
      "~/…/w-ambtools/h/code/telemetry",
    );
  });
  it("a long answer shows its start and says the rest is in bise", () => {
    const long = Array.from({ length: 40 }, (_, i) => `line ${i}`).join("\n");
    expect(answerLines(long).cut).toBe(true);
    expect(answerLines(long).shown.split("\n").length).toBe(ANSWER_MAX_LINES);
    expect(answerLines("short").cut).toBe(false);
  });
});

describe("the spotlight", () => {
  it("a waiting row's trailing item: its project, or its key once it is the active row (designer m_25150)", () => {
    const ui = render(
      <Spotlight data={D} onAsk={vi.fn()} onOpen={vi.fn()} onClose={vi.fn()} />,
    );
    const row = ui.getByTestId("spot.row.card.c1");
    const trail = row.querySelector(".k")?.textContent;
    if (row.getAttribute("aria-selected") === "true") {
      expect(trail).toBe("⏎ answer");
    } else {
      expect(trail).not.toBe("⏎ answer");
      expect(trail ?? "").not.toBe("");
    }
  });
});

/** A Storage in memory: the spotlight's draft and history (keep.ts in the app). */
const memStore = (): Store & { m: Map<string, string> } => {
  const m = new Map<string, string>();
  return {
    m,
    getItem: (k) => m.get(k) ?? null,
    setItem: (k, v) => void m.set(k, v),
    removeItem: (k) => void m.delete(k),
  };
};

type Props = Partial<Parameters<typeof Spotlight>[0]>;

/** The spotlight with the app's composer field: his typing goes in through the editor, his keys
 *  as keydowns on it (ProseMirror's handleKeyDown, the spotlight's keyFirst first). */
function spot(over: Props = {}) {
  const onAsk = vi.fn();
  const onOpen = vi.fn();
  const onClose = vi.fn();
  const store = memStore();
  const ui = render(
    <Spotlight data={D} onAsk={onAsk} onOpen={onOpen} onClose={onClose} store={store} {...over} />,
  );
  const field = (): HTMLElement => ui.getByTestId("spot.input.field");
  const editor = (): Editor => (field() as unknown as { editor: Editor }).editor;
  const type = async (s: string): Promise<void> => {
    await act(async () => {
      const v = editor().view;
      s.split("\n").forEach((line, i) => {
        const br = v.state.schema.nodes.hardBreak;
        if (i > 0 && br !== undefined) v.dispatch(v.state.tr.replaceSelectionWith(br.create()));
        if (line !== "") v.dispatch(v.state.tr.insertText(line));
      });
      await Promise.resolve();
    });
  };
  const caret = async (offset: number): Promise<void> => {
    await act(async () => {
      const v = editor().view;
      v.dispatch(v.state.tr.setSelection(TextSelection.create(v.state.doc, offset + 1)));
      await Promise.resolve();
    });
  };
  const press = async (key: string, mods: { metaKey?: boolean; shiftKey?: boolean } = {}): Promise<KeyboardEvent> => {
    const e = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...mods });
    await act(async () => {
      field().dispatchEvent(e);
      await Promise.resolve();
    });
    return e;
  };
  const text = (): string => field().textContent;
  const lit = (): string | null =>
    ui.container.querySelector('[data-testid^="spot.row."][aria-selected="true"]')?.getAttribute("data-testid") ?? null;
  const keys = (): string => ui.getByTestId("spot.frame").textContent;
  return { ui, onAsk, onOpen, onClose, store, field, type, caret, press, text, lit, keys };
}

describe("the spotlight's composer (Gabriel's v4, spotlight-composer#choix)", () => {
  it("the app's composer field, one node while the list changes under it", async () => {
    const s = spot();
    const f = s.field();
    expect(f.getAttribute("contenteditable")).toBe("true");
    for (const w of ["w", "h", "y is the hot query slow"]) {
      await s.type(w);
      expect(s.field()).toBe(f);
    }
  });

  it("⏎ on a search opens its first result; the key line names it", async () => {
    const s = spot();
    await s.type("perf");
    expect(s.lit()).toBe("spot.row.agent.tel.perf");
    expect(s.keys()).toContain("⏎ open perf");
    await s.press("Enter");
    expect(s.onOpen).toHaveBeenCalledWith(expect.objectContaining({ id: "agent.tel.perf" }));
    expect(s.onAsk).not.toHaveBeenCalled();
  });

  it("⏎ on a message sends it to bise, the list gone when nothing is named; the field empties", async () => {
    const s = spot();
    await s.type("can you check this against the POC?");
    expect(s.ui.container.querySelector('[data-testid^="spot.row."]')).toBeNull();
    expect(s.keys()).toContain("⏎ send to bise");
    await s.press("Enter");
    expect(s.onAsk).toHaveBeenCalledWith("can you check this against the POC?", undefined);
    expect(s.onOpen).not.toHaveBeenCalled();
    expect(s.text()).toBe("");
  });

  it("⇧⏎ is a new line, and a line break makes a message: ⏎ then sends, even after a name", async () => {
    const s = spot();
    await s.type("perf");
    const e = await s.press("Enter", { shiftKey: true });
    expect(e.defaultPrevented).toBe(true);
    expect(s.onOpen).not.toHaveBeenCalled();
    await s.type("check the sort");
    await s.press("Enter");
    expect(s.onAsk).toHaveBeenCalledWith("perf\ncheck the sort", undefined);
    expect(s.onOpen).not.toHaveBeenCalled();
  });

  it("⌘⏎ in his words sends to bise, even on a search", async () => {
    const s = spot();
    await s.type("perf");
    await s.press("Enter", { metaKey: true });
    expect(s.onAsk).toHaveBeenCalledWith("perf", undefined);
    expect(s.onOpen).not.toHaveBeenCalled();
  });

  it("↓ on his last character goes into the list; elsewhere it is the caret's", async () => {
    const s = spot();
    await s.type("can perf check this?\nthe sort");
    expect(s.lit(), "a message lights no row").toBeNull();
    // the caret on the first line: ↓ is the caret's (the field moves it), not the list's
    await s.caret(3);
    const mid = await s.press("ArrowDown");
    expect(mid.defaultPrevented).toBe(false);
    expect(s.lit()).toBeNull();
    // on the last line, not at its end: to its end, the list still out
    await s.caret(23);
    await s.press("ArrowDown");
    expect(s.lit()).toBeNull();
    // on the last character: into the list, its first row lit
    await s.press("ArrowDown");
    expect(s.lit()).toBe("spot.row.agent.tel.perf");
    expect(s.keys()).toContain("↑ on the first row: the text");
  });

  it("↑ on the list's first row comes back to his words; a letter typed in the list comes back with it", async () => {
    const s = spot();
    await s.type("tele");
    await s.press("ArrowDown");
    expect(s.lit()).toBe("spot.row.agent.tel.alerts");
    await s.press("ArrowDown");
    expect(s.lit()).toBe("spot.row.agent.tel.perf");
    await s.press("ArrowUp");
    await s.press("ArrowUp");
    expect(s.keys(), "back in his words").toContain("⏎ open alerts");
    await s.press("ArrowDown");
    const letter = await s.press("m");
    expect(letter.defaultPrevented, "the letter goes in his words").toBe(false);
    expect(s.keys()).toContain("↓ the list");
  });

  it("esc in the list comes back to his words; in his words it closes, the draft kept for the next fn space, selected", async () => {
    const s = spot();
    await s.type("tele");
    await s.press("ArrowDown");
    await s.press("Escape");
    expect(s.onClose).not.toHaveBeenCalled();
    expect(s.keys()).toContain("↓ the list");
    await s.press("Escape");
    expect(s.onClose).toHaveBeenCalledTimes(1);
    expect(s.text(), "esc keeps his words").toBe("tele");
    // the draft lives in the store: the next spotlight shows it, all selected (typing replaces it)
    await act(async () => {
      await new Promise((r) => setTimeout(r, 450));
    });
    const store = s.store;
    cleanup();
    const again = spot({ store });
    expect(again.text()).toBe("tele");
    const sel = again.field() as unknown as { editor: Editor };
    expect(sel.editor.state.selection.empty).toBe(false);
  });

  it("⏎ on a row under a message opens it with his words as its draft; ⌘⏎ sends them to it", async () => {
    const onOpenWith = vi.fn();
    const onSendTo = vi.fn();
    const s = spot({ onOpenWith, onSendTo });
    await s.type("can perf check this against the POC?");
    await s.press("ArrowDown");
    expect(s.keys()).toContain("⏎ open with your words");
    await s.press("Enter");
    expect(onOpenWith).toHaveBeenCalledTimes(1);
    const [item, entry] = onOpenWith.mock.calls[0] as [{ id: string }, string];
    expect(item.id).toBe("agent.tel.perf");
    expect(outgoingOf(entry)).toEqual({ text: "can perf check this against the POC?" });
    expect(s.onAsk).not.toHaveBeenCalled();
    expect(s.text(), "his words went with it").toBe("");
    await s.type("can perf look again?");
    await s.press("ArrowDown");
    await s.press("Enter", { metaKey: true });
    expect(onSendTo).toHaveBeenCalledWith(expect.objectContaining({ id: "agent.tel.perf" }), { text: "can perf look again?" });
    expect(s.onAsk).not.toHaveBeenCalled();
  });

  it("↑ in the empty field brings his last message back", async () => {
    const s = spot();
    await s.type("why is it slow?");
    await s.press("Enter");
    expect(s.text()).toBe("");
    await s.press("ArrowUp");
    expect(s.text()).toBe("why is it slow?");
  });

  it("a row that stays listed keeps its element when typing moves it to another group", async () => {
    const s = spot();
    const perf = s.ui.getByTestId("spot.row.agent.tel.perf");
    await s.type("p");
    expect(s.ui.getByTestId("spot.group.found")).toBeTruthy();
    expect(s.ui.getByTestId("spot.row.agent.tel.perf")).toBe(perf);
  });

  it("⌘1-9 opens a project from his words", async () => {
    const s = spot();
    await s.type("draft");
    await s.press("2", { metaKey: true });
    expect(s.onOpen).toHaveBeenCalledWith(expect.objectContaining({ id: "env.shop" }));
  });
});

describe("the routing hold", () => {
  it('shows "→ bise · <project> · N s to change" at once; esc keeps it with bise, ⏎ on another project corrects it', async () => {
    const onRouteKeep = vi.fn();
    const onRouteTo = vi.fn();
    const route = { rid: 7, to: "tel", name: "telemetry", until: Date.now() + 2000 };
    const s = spot({ route, onRouteKeep, onRouteTo });
    expect(s.ui.getByTestId("spot.route").textContent).toMatch(/^→ bise · telemetry · 2 s to change/u);
    expect(s.ui.queryByTestId("spot.row.env.tel")).toBeNull();
    await s.press("Enter");
    expect(onRouteTo).toHaveBeenCalledWith("shop");
    await s.press("Escape");
    expect(onRouteKeep).toHaveBeenCalledTimes(1);
    expect(s.onClose).not.toHaveBeenCalled();
  });
});

describe("his ask shows where it is (R18 on 723648da)", () => {
  it("asked, no word back yet: 'asked bise' dim with the ∿; the first words take its place", () => {
    const ui = render(
      <Spotlight
        data={D}
        answer={{ text: "", done: false }}
        onAsk={vi.fn()}
        onOpen={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    const a = ui.getByTestId("spot.answer");
    expect(a.dataset.state).toBe("asked");
    expect(a.className).toContain("note");
    expect(a.textContent).toContain("asked bise · the answer shows here");
    expect(ui.getByTestId("spot.answer.text.sign")).toBeTruthy();
    ui.rerender(
      <Spotlight
        data={D}
        answer={{ text: "The cold bench", done: false }}
        onAsk={vi.fn()}
        onOpen={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(ui.getByTestId("spot.answer").dataset.state).toBe("live");
    expect(ui.getByTestId("spot.answer").className).not.toContain("note");
    expect(ui.getByTestId("spot.answer").textContent).not.toContain(
      "asked bise",
    );
  });

  it("sent to another project: a dim line says where its answer is, no ∿", () => {
    const ui = render(
      <Spotlight
        data={D}
        answer={{
          text: "sent to telemetry · its answer is in telemetry's thread",
          done: true,
          note: true,
        }}
        onAsk={vi.fn()}
        onOpen={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    const a = ui.getByTestId("spot.answer");
    expect(a.dataset.state).toBe("went");
    expect(a.className).toContain("note");
    expect(ui.queryByTestId("spot.answer.text.sign")).toBeNull();
  });
});

describe("start an agent from the spotlight (bar A.1)", () => {
  it("'/new …' and 'start an agent: …' give the brief; anything else, or no brief, is not one", () => {
    expect(newBrief("/new fix the flaky login test")).toBe(
      "fix the flaky login test",
    );
    expect(newBrief("  Start an agent: profile the hub ")).toBe(
      "profile the hub",
    );
    expect(newBrief("start an agent profile the hub")).toBe("profile the hub");
    expect(newBrief("/new")).toBeUndefined();
    expect(newBrief("start an agent:  ")).toBeUndefined();
    expect(newBrief("/newsletter draft")).toBeUndefined();
    expect(newBrief("please start an agent: x")).toBeUndefined();
  });

  it("the list is one row that starts it", () => {
    const l = spotList(D, "/new profile the hub");
    expect(l.mode).toBe("new");
    expect(flat(l)).toEqual([
      { kind: "new", id: "new", brief: "profile the hub" },
    ]);
  });

  it("⏎ starts it with the brief (not sent to bise), the field clears; with no onNew it goes to bise as words", async () => {
    const onNew = vi.fn();
    const s = spot({ onNew });
    await s.type("start an agent: profile the hub");
    expect(s.ui.getByTestId("spot.row.new").textContent).toContain("start an agent");
    await s.press("Enter");
    expect(onNew).toHaveBeenCalledWith("profile the hub");
    expect(s.onAsk).not.toHaveBeenCalled();
    expect(s.text()).toBe("");
    cleanup();
    const s2 = spot();
    await s2.type("/new profile the hub");
    await s2.press("Enter");
    expect(s2.onAsk).toHaveBeenCalledWith("/new profile the hub", undefined);
  });
});

describe("spot-shot: the window he was on, and his long words", () => {
  it("his words wrap: the field grows to a cap, then scrolls down, never sideways", async () => {
    const { readFileSync } = await import("node:fs");
    // vitest runs in apps/desktop (happy-dom's import.meta.url is not a file URL)
    const SPOT_CSS = readFileSync(`${process.cwd()}/src/spotlight/spotlight.css`, "utf8");
    const rule = /\.b-spot-field \.b-field\.ProseMirror \{([^}]*)\}/u.exec(SPOT_CSS)?.[1] ?? "";
    for (const d of ["max-height:", "overflow-x: hidden", "overflow-y: auto", "overflow-wrap: anywhere", "white-space: pre-wrap"])
      expect(rule, d).toContain(d);
  });

  it("the still's chip comes first; ✕ and ⌫ at the start of his words drop it; no still and Screen Recording off: why", async () => {
    const drop = vi.fn();
    const shot = { thumb: "file:///i/a.png", name: "Safari · stripe.com", title: "Safari · stripe.com/pricing" };
    const s = spot({ shot, onShotDrop: drop });
    const chip = s.ui.getByTestId("spot.shot");
    expect(chip.textContent).toContain("Safari · stripe.com");
    expect(chip.getAttribute("title")).toBe("Safari · stripe.com/pricing");
    fireEvent.click(s.ui.getByTestId("spot.shot.drop"));
    expect(drop).toHaveBeenCalledTimes(1);
    await s.press("Backspace");
    expect(drop).toHaveBeenCalledTimes(2);
    // ⌫ inside his words is the field's
    await s.type("ab");
    await s.press("Backspace");
    expect(drop).toHaveBeenCalledTimes(2);
    expect(s.ui.queryByTestId("spot.noshot"), "a still: nothing to explain").toBeNull();
    cleanup();
    const open = vi.fn();
    const off = spot({ shot: null, shotOff: { onOpen: open } });
    expect(off.ui.getByTestId("spot.noshot").textContent).toBe("no screenshot · Screen Recording is off");
    fireEvent.click(off.ui.getByTestId("spot.noshot"));
    expect(open).toHaveBeenCalledTimes(1);
  });
});

describe("the ❝ chip: his selection read at fn space (Gabriel's v4, architect m_53938)", () => {
  const q = { text: "the card number field loses focus when the page scrolls on Safari", app: "Safari" };

  it("its words: cut at 40 with …, its app only when it is not the app in front", () => {
    expect(quoteChip(q, "Safari")).toBe("❝ the card number field loses focus when…");
    expect(quoteChip(q, "Chrome")).toBe("❝ the card number field loses focus when… · Safari");
    expect(quoteChip({ text: "  p99\n 412 ms ", app: "Safari" }, null)).toBe("❝ p99 412 ms · Safari");
  });

  it("first on the chips' line, before the still; its whole text on hover", () => {
    const shot = { thumb: "file:///i/a.png", name: "Safari · stripe.com", title: "Safari · stripe.com/pricing" };
    const s = spot({ quote: q, frontApp: "Safari", shot });
    const chips = s.ui.container.querySelector(".b-spot-chips");
    expect([...(chips?.children ?? [])].map((c) => c.getAttribute("data-kind"))).toEqual(["quote", "shot"]);
    expect(s.ui.getByTestId("spot.quote").getAttribute("title")).toBe(q.text);
  });

  it("sent first with his message, as a quote part from its app; ⌘⏎ on a row too", async () => {
    const s = spot({ quote: q });
    await s.type("why does this happen?");
    await s.press("Enter");
    expect(s.onAsk).toHaveBeenCalledWith("why does this happen?", [
      { kind: "quote", from: "Safari", text: q.text },
      { kind: "text", text: "why does this happen?" },
    ]);
    expect(withQuote({ text: "x" }, null)).toEqual({ text: "x" });
  });

  it("✕ and ⌫ at the start of his words forget it; ⌫ takes the still first", async () => {
    const drop = vi.fn();
    const shotDrop = vi.fn();
    const shot = { thumb: "file:///i/a.png", name: "Safari", title: "Safari" };
    const s = spot({ quote: q, onQuoteDrop: drop, shot, onShotDrop: shotDrop });
    await s.press("Backspace");
    expect([shotDrop.mock.calls.length, drop.mock.calls.length]).toEqual([1, 0]);
    s.ui.rerender(<Spotlight data={D} onAsk={s.onAsk} onOpen={s.onOpen} onClose={s.onClose} store={s.store} quote={q} onQuoteDrop={drop} />);
    await s.press("Backspace");
    expect(drop).toHaveBeenCalledTimes(1);
    fireEvent.click(s.ui.getByTestId("spot.quote.drop"));
    expect(drop).toHaveBeenCalledTimes(2);
    // ⌫ inside his words is the field's
    await s.type("ab");
    await s.press("Backspace");
    expect(drop).toHaveBeenCalledTimes(2);
  });

  it("⌘Z right after he forgot it brings it back; once he typed, ⌘Z is the field's", async () => {
    const back = vi.fn();
    const s = spot({ quote: q, onQuoteDrop: vi.fn() });
    await s.press("Backspace");
    s.ui.rerender(<Spotlight data={D} onAsk={s.onAsk} onOpen={s.onOpen} onClose={s.onClose} store={s.store} quote={null} onQuoteBack={back} />);
    const e = await s.press("z", { metaKey: true });
    expect(back).toHaveBeenCalledTimes(1);
    expect(e.defaultPrevented).toBe(true);
    cleanup();
    const t = spot({ quote: q, onQuoteDrop: vi.fn() });
    await t.press("Backspace");
    await t.type("a");
    t.ui.rerender(<Spotlight data={D} onAsk={t.onAsk} onOpen={t.onOpen} onClose={t.onClose} store={t.store} quote={null} onQuoteBack={back} />);
    await t.press("z", { metaKey: true });
    expect(back).toHaveBeenCalledTimes(1);
  });
});
