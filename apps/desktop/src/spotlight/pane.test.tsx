// @vitest-environment happy-dom
// The thread pane (page spotlight-reponse, Gabriel's pick B): its keys, each one failing without
// its line in ThreadPane's keyFirst, and the capsule's two inputs (Capsule.tsx): a follow-up opens
// the pane with the focus in it, the answer leaves the left field, tab and esc go back and forth,
// ✕ ends it with the focus back left, a card answered in place keeps the pane.
import { act, cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Editor } from "@tiptap/core";
import type { Store } from "@/composer/drafts";
import { Capsule, type CapsuleProps, type PaneProps } from "@/capsule/Capsule";
import type { PaneRow } from "@/capsule/exchange";
import { ThreadPane } from "./ThreadPane";

afterEach(cleanup);

const memStore = (): Store => {
  const m = new Map<string, string>();
  return {
    getItem: (k) => m.get(k) ?? null,
    setItem: (k, v) => void m.set(k, v),
    removeItem: (k) => void m.delete(k),
  };
};

const ROWS: readonly PaneRow[] = [
  { kind: "you", pos: 3, text: "why is the query slow?" },
  { kind: "bise", pos: 4, text: "the **index** is missing", live: false },
  { kind: "you", pos: 5, text: "add it?" },
  { kind: "bise", pos: -1, text: "adding `idx_user`", live: true },
];

const editorOf = (el: HTMLElement): Editor => (el as unknown as { editor: Editor }).editor;

const press = async (el: HTMLElement, key: string, mods: { metaKey?: boolean; shiftKey?: boolean } = {}): Promise<KeyboardEvent> => {
  const e = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...mods });
  await act(async () => {
    el.dispatchEvent(e);
    await Promise.resolve();
  });
  return e;
};

function pane(rows: readonly PaneRow[] = ROWS) {
  const p = { onSend: vi.fn(), onOpen: vi.fn(), onEnd: vi.fn(), onBack: vi.fn(), onCopy: vi.fn() };
  const ui = render(<ThreadPane rows={rows} {...p} store={memStore()} />);
  const field = (): HTMLElement => ui.getByTestId("spot.pane.input.field");
  return { ui, ...p, field };
}

describe("the thread pane (pick B)", () => {
  it("its rows: his bubbles, bise's answers as the thread draws them, the live one with its ∿", () => {
    const s = pane();
    expect(s.ui.getByTestId("spot.pane").textContent).toContain("bise · this thread");
    expect(s.ui.getAllByTestId("spot.pane.row.you").map((r) => r.textContent)).toEqual(["why is the query slow?", "add it?"]);
    const bise = s.ui.getAllByTestId("spot.pane.row.bise");
    expect(bise[0]?.querySelector("strong")?.textContent).toBe("index");
    expect(bise.map((r) => r.getAttribute("data-live"))).toEqual(["0", "1"]);
    // the live answer writes in as the thread's does (its words come in, the ∿ rides its end)
    expect(bise[1]?.textContent).toMatch(/^adding.*∿$/u);
  });

  it("tab and esc go back to the left field; ⌘K too", async () => {
    const s = pane();
    expect((await press(s.field(), "Tab")).defaultPrevented).toBe(true);
    await press(s.field(), "Escape");
    await press(s.field(), "k", { metaKey: true });
    expect(s.onBack).toHaveBeenCalledTimes(3);
  });

  it("⌘⏎ opens this thread in bise; ⌘W ends it, as ✕ does", async () => {
    const s = pane();
    await press(s.field(), "Enter", { metaKey: true });
    expect(s.onOpen).toHaveBeenCalledTimes(1);
    expect(s.onSend).not.toHaveBeenCalled();
    await press(s.field(), "w", { metaKey: true });
    expect(s.onEnd).toHaveBeenCalledTimes(1);
    s.ui.getByTestId("spot.pane.end").click();
    expect(s.onEnd).toHaveBeenCalledTimes(2);
    expect(s.ui.getByTestId("spot.pane.end").getAttribute("title")).toBe("end this thread · ⌘W");
  });

  it("⌘C with nothing selected copies the last answer; no answer yet: the key stays the field's", async () => {
    const s = pane();
    expect((await press(s.field(), "c", { metaKey: true })).defaultPrevented).toBe(true);
    expect(s.onCopy).toHaveBeenCalledWith("adding `idx_user`");
    cleanup();
    const t = pane([{ kind: "you", pos: 3, text: "hi" }, { kind: "wait" }]);
    expect((await press(t.field(), "c", { metaKey: true })).defaultPrevented).toBe(false);
    expect(t.onCopy).not.toHaveBeenCalled();
  });

  it("⏎ sends his follow-up from its own composer, ⇧⏎ a new line", async () => {
    const s = pane();
    await act(async () => {
      const v = editorOf(s.field()).view;
      v.dispatch(v.state.tr.insertText("and the other table?"));
      await Promise.resolve();
    });
    await press(s.field(), "Enter", { shiftKey: true });
    expect(s.onSend).not.toHaveBeenCalled();
    await press(s.field(), "Enter");
    expect(s.onSend).toHaveBeenCalledTimes(1);
    expect((s.onSend.mock.calls[0]?.[0] as { text: string }).text).toMatch(/^and the other table\?/u);
  });
});

describe("his bubbles in the pane (designer m_66238)", () => {
  it("from his parts: his words, his image as the thread's chip with its caption; never the hub's hint nor an image tag", () => {
    const tagged = '<image b64="/Users/x/.bise/images/ab.b64" caption="Zed · ai-docs — tools.ts">\nC\'est quoi comme language\n\n[from the capsule: answer in one short line]';
    const s = pane([
      {
        kind: "you",
        pos: 3,
        text: tagged,
        parts: [
          { kind: "image", name: "[Image #1]", source: "/Users/x/shot.png", mime: "image/png", b64_file: "/Users/x/.bise/images/ab.b64", caption: "Zed · ai-docs — tools.ts" },
          { kind: "text", text: "C'est quoi comme language" },
          { kind: "hint", of: "capsule", text: "\n\n[from the capsule: answer in one short line]" },
        ],
      },
      // an older hub's entry, no parts: the hint cut from its text
      { kind: "you", pos: 5, text: "C'est populaire\n\n[from the capsule: answer in one short line]" },
    ]);
    const rows = s.ui.getAllByTestId("spot.pane.row.you");
    const all = rows.map((r) => r.textContent).join(" | ");
    expect(all).not.toContain("[from the capsule");
    expect(all).not.toContain("b64=");
    expect(rows[0]?.textContent).toContain("C'est quoi comme language");
    expect(s.ui.getByTestId("spot.pane.row.you.0.image.1")).toBeTruthy();
    expect(s.ui.getByTestId("spot.pane.row.you.0.image.1.caption").textContent).toBe("Zed · ai-docs — tools.ts");
    expect(rows[1]?.textContent).toBe("C'est populaire");
  });
});

describe("the capsule's two inputs (pick B)", () => {
  const base: CapsuleProps = {
    view: { kind: "spot" }, spot: { agents: [], projects: [], cards: [] }, answer: { text: "the **index** is missing", done: true }, heard: "", level: 0,
    card: () => <div data-testid="the-card">card</div>, route: null, onRouteKeep: vi.fn(), onRouteTo: vi.fn(), onAsk: vi.fn(), onOpen: vi.fn(), onOpenWindow: vi.fn(), onClose: vi.fn(), onQuit: null, onSize: vi.fn(),
  };
  const paneOf = (over: Partial<PaneProps> = {}): PaneProps => ({ rows: ROWS, onSend: vi.fn(), onOpen: vi.fn(), onEnd: vi.fn(), onCopy: vi.fn(), ...over });
  const inPane = (): boolean => document.activeElement?.closest("[data-pane]") != null;
  const inLeft = (): boolean => document.activeElement?.closest('[data-testid="spot.input"]') != null || document.activeElement?.getAttribute("data-testid") === "spot.input.field";

  it("the first answer stays under the left field; a follow-up opens the pane, the answer moves into it, the focus too", async () => {
    const ui = render(<Capsule {...base} />);
    expect(ui.getByTestId("spot.answer").textContent).toContain("index");
    expect(ui.queryByTestId("spot.pane")).toBeNull();
    await act(async () => {
      ui.rerender(<Capsule {...base} pane={paneOf()} />);
      await Promise.resolve();
    });
    expect(ui.queryByTestId("spot.answer")).toBeNull();
    expect(ui.getByTestId("spot.pane")).toBeTruthy();
    expect(ui.container.querySelector(".b-cap-glass.with-pane")).not.toBeNull();
    expect(inPane()).toBe(true);
  });

  it("tab from the left field goes to the pane's composer, tab there comes back; without the pane tab stays the field's", async () => {
    const ui = render(<Capsule {...base} pane={paneOf()} />);
    await act(async () => {
      await Promise.resolve();
    });
    await press(ui.getByTestId("spot.pane.input.field"), "Tab");
    expect(inLeft()).toBe(true);
    await press(ui.getByTestId("spot.input.field"), "Tab");
    expect(inPane()).toBe(true);
    await press(ui.getByTestId("spot.pane.input.field"), "Escape");
    expect(inLeft()).toBe(true);
    // esc in the left field closes the spotlight (the pane stays the exchange's)
    await press(ui.getByTestId("spot.input.field"), "Escape");
    expect(base.onClose).toHaveBeenCalled();
    cleanup();
    const solo = render(<Capsule {...base} />);
    expect((await press(solo.getByTestId("spot.input.field"), "Tab")).defaultPrevented).toBe(false);
  });

  it("fn space with the pane's composer focused: the root gets the act that moves the focus left", async () => {
    const got: { act: (() => boolean) | null } = { act: null };
    const ui = render(<Capsule {...base} pane={paneOf()} onFocusLeft={(a) => { got.act = a; }} />);
    await act(async () => {
      await Promise.resolve();
    });
    expect(inPane()).toBe(true);
    let moved = false;
    await act(async () => {
      moved = got.act?.() ?? false;
      await Promise.resolve();
    });
    expect(moved).toBe(true);
    expect(inLeft()).toBe(true);
    // the left field had it: the key does what it did (fn space closes)
    expect(got.act?.()).toBe(false);
    ui.unmount();
    expect(got.act).toBeNull();
  });

  it("the pane opening: the window's next size says 'open' (main glides it re-centred), the glass is the whole view, the content comes in", async () => {
    const onSize = vi.fn();
    const ui = render(<Capsule {...base} onSize={onSize} />);
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(onSize.mock.calls.every((c) => c[3] === undefined)).toBe(true);
    onSize.mockClear();
    await act(async () => {
      ui.rerender(<Capsule {...base} onSize={onSize} pane={paneOf()} />);
      await new Promise((r) => setTimeout(r, 0));
    });
    const morphs = (onSize.mock.calls as unknown[][]).filter((c) => c[3] !== undefined);
    expect(morphs.map((c) => c[3])).toEqual(["open"]);
    expect(ui.getByTestId("spot.pane").hasAttribute("data-enter")).toBe(true);
    // the glass main centres: the whole view (spotlight + pane), never the left part alone
    const glass = ui.container.querySelector("[data-glass]")?.getBoundingClientRect();
    expect(morphs[0]?.[2]).toMatchObject({ width: Math.round(glass?.width ?? -1) });
  });

  it("✕: the pane's content fades out 80 ms, then it closes; the size after says 'close'", async () => {
    vi.useFakeTimers();
    try {
      const onEnd = vi.fn();
      const onSize = vi.fn();
      const ui = render(<Capsule {...base} onSize={onSize} pane={paneOf({ onEnd })} />);
      act(() => {
        ui.getByTestId("spot.pane.end").click();
      });
      expect(onEnd).not.toHaveBeenCalled();
      expect(ui.getByTestId("spot.pane").hasAttribute("data-leaving")).toBe(true);
      expect(inLeft()).toBe(true);
      act(() => {
        vi.advanceTimersByTime(80);
      });
      expect(onEnd).toHaveBeenCalledTimes(1);
      onSize.mockClear();
      act(() => {
        ui.rerender(<Capsule {...base} onSize={onSize} />);
      });
      await act(async () => {
        await vi.runAllTimersAsync();
      });
      expect((onSize.mock.calls as unknown[][]).filter((c) => c[3] !== undefined).map((c) => c[3])).toEqual(["close"]);
    } finally {
      vi.useRealTimers();
    }
  });

  it("reduced motion: ✕ closes at once and the open plays no entrance", () => {
    const mm = vi.spyOn(window, "matchMedia").mockImplementation((q: string) => ({ matches: q.includes("reduced-motion"), media: q, onchange: null, addListener: vi.fn(), removeListener: vi.fn(), addEventListener: vi.fn(), removeEventListener: vi.fn(), dispatchEvent: () => false }) as MediaQueryList);
    try {
      const onEnd = vi.fn();
      const ui = render(<Capsule {...base} />);
      act(() => {
        ui.rerender(<Capsule {...base} pane={paneOf({ onEnd })} />);
      });
      expect(ui.getByTestId("spot.pane").hasAttribute("data-enter")).toBe(false);
      act(() => {
        ui.getByTestId("spot.pane.end").click();
      });
      expect(onEnd).toHaveBeenCalledTimes(1);
    } finally {
      mm.mockRestore();
    }
  });

  it("⏎ on a waiting card answers it in place: the card on the left, the pane stays", () => {
    const ui = render(<Capsule {...base} view={{ kind: "card", id: "tel.1" }} pane={paneOf()} />);
    expect(ui.getByTestId("the-card")).toBeTruthy();
    expect(ui.getByTestId("spot.pane")).toBeTruthy();
    // from the tray menu: the card alone, as before
    ui.rerender(<Capsule {...base} view={{ kind: "card", id: "tel.1", from: "menu" }} pane={paneOf()} />);
    expect(ui.queryByTestId("spot.pane")).toBeNull();
  });
});
