import { describe, expect, it } from "vitest";
import { cutWords, glyphColumn, pairKey, pairStart, pickerPlace, stepActive, typeahead, walkable } from "./picker";
import { pairPlace } from "./dropdown";

describe("ds/picker", () => {
  const groups = [
    { id: "a", rows: [{ id: "1", name: "one" }, { id: "2", name: "two", disabled: true }] },
    { id: "b", rows: [{ id: "3", name: "three" }, { id: "4", name: "four" }] },
  ];

  it("walks the rows that aren't disabled", () => {
    expect(walkable(groups)).toEqual(["1", "3", "4"]);
  });

  it("↑↓ wrap, home and end, a page of 8", () => {
    const ids = walkable(groups);
    expect(stepActive(ids, "4", "ArrowDown")).toBe("1");
    expect(stepActive(ids, "1", "ArrowUp")).toBe("4");
    expect(stepActive(ids, null, "ArrowDown")).toBe("1");
    expect(stepActive(ids, null, "ArrowUp")).toBe("4");
    expect(stepActive(ids, "3", "Home")).toBe("1");
    expect(stepActive(ids, "1", "End")).toBe("4");
    expect(stepActive(ids, "1", "PageDown")).toBe("4");
    expect(stepActive(ids, "4", "PageUp")).toBe("1");
    expect(stepActive(ids, "1", "a")).toBeNull();
    expect(stepActive([], null, "ArrowDown")).toBeNull();
  });

  it("sits 8 px from its anchor, on its trigger's edge, above unless below has more room, its height fitted", () => {
    const win = { width: 1100, height: 820 };
    const start = pickerPlace({ top: 700, bottom: 780 }, { left: 300, right: 360 }, "start", win);
    expect(start.style).toEqual({ position: "fixed", left: 300, bottom: 128 });
    // 684 px of room above, but never taller than half the window (his m_26194)
    expect(start.maxHeight).toBe(410);
    const end = pickerPlace({ top: 700, bottom: 780 }, { left: 900, right: 1000 }, "end", win);
    expect(end.style).toEqual({ position: "fixed", right: 100, bottom: 128 });
    expect(pickerPlace({ top: 700, bottom: 780 }, { left: 2, right: 40 }, "start", win).style).toMatchObject({ left: 8 });
    // little room above, more below: it flips under the anchor
    const flip = pickerPlace({ top: 100, bottom: 130 }, { left: 10, right: 60 }, "start", win);
    expect(flip.style).toEqual({ position: "fixed", left: 10, top: 138 });
    expect(flip.maxHeight).toBe(410);
    // a short window: never taller than the room
    expect(pickerPlace({ top: 300, bottom: 380 }, { left: 10, right: 60 }, "start", { width: 800, height: 420 }).maxHeight).toBe(210);
    expect(pickerPlace({ top: 700, bottom: 780 }, { left: 10, right: 60 }, "start", { width: 800, height: 1600 }).maxHeight).toBe(684);
  });

  it("a menu opens below its trigger unless the room there is short, on the left edge unless a 420 px picker would pass the window", () => {
    const win = { width: 1100, height: 820 };
    const down = pickerPlace({ top: 100, bottom: 128 }, { left: 40, right: 140 }, "auto", win, "below");
    expect(down.style).toEqual({ position: "fixed", left: 40, top: 136 });
    expect(down.maxHeight).toBe(410);
    // near the window's foot: above it
    expect(pickerPlace({ top: 700, bottom: 728 }, { left: 40, right: 140 }, "auto", win, "below").style).toMatchObject({ bottom: 128 });
    // near its right edge: on the trigger's right edge
    expect(pickerPlace({ top: 100, bottom: 128 }, { left: 900, right: 1000 }, "auto", win, "below").style).toMatchObject({ right: 100 });
  });

  it("a letter jumps to the next row that starts with it, letters typed together to the first that starts with them", () => {
    const g = [{ id: "g", rows: [{ id: "amb-feed", name: "amb-feed" }, { id: "amb-mac", name: "amb-mac" }, { id: "main", name: "main" }, { id: "off", name: "agents", disabled: true }] }];
    expect(typeahead(g, null, "a")).toBe("amb-feed");
    expect(typeahead(g, "amb-feed", "a")).toBe("amb-mac");
    expect(typeahead(g, "amb-mac", "a")).toBe("amb-feed");
    expect(typeahead(g, "amb-feed", "amb-m")).toBe("amb-mac");
    expect(typeahead(g, null, "M")).toBe("main");
    expect(typeahead(g, null, "z")).toBeNull();
  });

  it("shows the glyph column only when the list mixes kinds", () => {
    const g = (glyphs: (string | undefined)[]) => [{ id: "g", rows: glyphs.map((glyph, i) => ({ id: String(i), name: "x", glyph })) }];
    expect(glyphColumn(g(["/", "/"]))).toBe(false);
    expect(glyphColumn(g([undefined, undefined]))).toBe(false);
    expect(glyphColumn(g(["@", undefined]))).toBe(true);
    expect(glyphColumn(g(["@", "↗"]))).toBe(true);
  });

  it("cuts after a whole word with one '…'", () => {
    const fits = (n: number) => (t: string) => t.length <= n;
    expect(cutWords("an agent reads the changes", fits(40))).toBe("an agent reads the changes");
    expect(cutWords("an agent reads the changes", fits(16))).toBe("an agent reads…");
    expect(cutWords("an agent, reads", fits(10))).toBe("an agent…");
    expect(cutWords("unbreakable", fits(3))).toBe("unbreakable…");
  });

  it("tries a long text's cuts by halving, the same cut as word by word (at-perf: each try is a layout)", () => {
    const long = Array.from({ length: 400 }, (_, i) => `word${String(i)}${i % 7 === 0 ? "," : ""}`).join(" ");
    const slow = (text: string, fits: (t: string) => boolean): string => {
      if (fits(text)) return text;
      const words = text.split(/(?<=\s)/u);
      for (let n = words.length - 1; n >= 1; n -= 1) {
        const t = `${words.slice(0, n).join("").replace(/[\s,;:·]+$/u, "")}…`;
        if (fits(t)) return t;
      }
      return `${(words[0] ?? "").trim()}…`;
    };
    for (const room of [3, 10, 37, 60, 200, 2000, 5000]) {
      let tries = 0;
      const fits = (t: string) => {
        tries += 1;
        return t.length <= room;
      };
      expect(cutWords(long, fits)).toBe(slow(long, (t) => t.length <= room));
      expect(tries).toBeLessThanOrEqual(11);
    }
  });
});

describe("ds/picker · PickerPair (designer m_57374)", () => {
  const sides = [
    { id: "A", name: "Anthropic", rows: [{ id: "a/opus", name: "opus" }, { id: "a/sonnet", name: "sonnet", current: true }] },
    { id: "M", name: "Mistral", rows: [{ id: "m/small", name: "small" }] },
  ];
  const top = { id: "(fallback)", name: "Same as bise · opus" };

  it("opens on the current row's side, that row active; else on the first side", () => {
    expect(pairStart({ top, sides })).toEqual({ col: "right", side: "A", on: "a/sonnet" });
    expect(pairStart({ top, sides: sides.map((s) => ({ ...s, rows: s.rows.map((r) => ({ ...r, current: false })) })) })).toEqual({ col: "left", side: "A", on: "A" });
  });

  it("↑↓ on the left choose the side as they go, → opens its rows, ← goes back, ⏎ picks", () => {
    const left = { col: "left" as const, side: "A", on: "A" };
    expect(pairKey({ top, sides }, left, "ArrowDown")?.at).toEqual({ col: "left", side: "M", on: "M" });
    // the top row is walked, but isn't a side: the side stays
    expect(pairKey({ top, sides }, left, "ArrowUp")?.at).toEqual({ col: "left", side: "A", on: "(fallback)" });
    expect(pairKey({ top, sides }, { col: "left", side: "A", on: "(fallback)" }, "Enter")?.pick).toBe("(fallback)");
    expect(pairKey({ top, sides }, left, "ArrowRight")?.at).toEqual({ col: "right", side: "A", on: "a/sonnet" });
    const right = { col: "right" as const, side: "A", on: "a/sonnet" };
    expect(pairKey({ top, sides }, right, "ArrowLeft")?.at).toEqual({ col: "left", side: "A", on: "A" });
    expect(pairKey({ top, sides }, right, "Enter")?.pick).toBe("a/sonnet");
    expect(pairKey({ top, sides }, right, "ArrowDown")?.at.on).toBe("a/opus");
  });

  it("opens with right edges aligned, from the left edge without room, one column with neither", () => {
    expect(pairPlace({ left: 700, right: 900 }, 1280)).toEqual({ align: "end", narrow: false });
    expect(pairPlace({ left: 100, right: 300 }, 1280)).toEqual({ align: "start", narrow: false });
    expect(pairPlace({ left: 200, right: 380 }, 560)).toEqual({ align: "end", narrow: true });
  });
});
