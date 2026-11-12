// Sheets · the grid (docs/plugin-apps.md, migration step 4; page xlsx-univer, architect m_37322,
// m_37328): a workbook in Univer's grid (office-sheet.js beside this file, built by
// scripts/bins.sh), where he resizes columns and rows, types values and formulas (recomputed by
// the grid), and his edits save themselves 2 s after the last one.
//
// - an xlsx: the grid's edits (editsBetween) are replayed on the file with the office SDK
//   (~kit/office/v1/office.js), which keeps what the grid never saw (charts, pivots), and its
//   bytes are saved;
// - a csv: one sheet from its rows (csv.js); a save is the sheet's values as CSV, in the file's
//   separator.
//
// A save is `ai.mistral.vibe/resources/write` through @bise/view, on the etag he last read or
// saved (ifMatch): the hub keeps one version per sitting (the kit's seq) and answers `conflict`
// when a version came in since; then the tab's head says so and offers Reload and Keep mine as
// a copy (the host keeps the body it was sent, apps-host m_60432), and his next edits still go
// to the hub on the same etag, so the copy is his latest. Nothing of that is drawn here: the save
// state is the head's facts line (A6b). A document he can't save (an older version, no write
// grant) opens read only.
//
// A note's anchor is the canonical JSON of the old viewer's cell anchor (`{"kind":"cell",
// "range":"B4:D9","sheet":"Q3"}`, architect m_60380); its mark is a box over its cells, kept there
// as the grid scrolls, or its sheet's tab when another sheet is shown.
//
// Pure helpers (plugins/sheets/test/book.test.mjs): cellKey, cellOfKey, notesOf, saver (the save
// loop without the DOM: the tests drive it with a fake grid and a fake write).

import { sheets, editsBetween } from "./~kit/office/v1/office.js";
import type { Outline, UWorkbook } from "./~kit/office/v1/office.js";
import type { Saved } from "./~kit/view/view.js";
import { parseCsv, cellOf, csvOf, sepOf } from "./csv.js";

/** How long after his last edit the file is saved. */
export const SAVE_AFTER_MS = 2000;
/** How long after a save that failed (the kit tried 3 times) it is tried again. */
export const RETRY_MS = 10_000;

type Book = Awaited<ReturnType<typeof sheets.open>>;
type GridPlace = { sheet: string; range: string | null };
type Grid = {
  snapshot(): UWorkbook;
  onChange(fn: () => void): void;
  place(): GridPlace;
  restore(p: GridPlace): void;
  rect(sheet: string, range: string): DOMRect | null;
  text(): string;
  onMove(fn: () => void): void;
  dark(on: boolean): void;
  dispose(): void;
};
type GridModule = { mountGrid(el: HTMLElement, data: UWorkbook, opts: { dark: boolean; readOnly?: boolean }): Grid };

/** Where the save loop is: nothing to save, edits waiting, saving, saved, a conflict (a version
 * came in: the head offers Reload and Keep mine as a copy), or failed (why, tried again later). */
export type SaveState = { kind: "idle" | "waiting" | "saving" | "saved" | "conflict" } | { kind: "failed"; why: string };

/** A cell range's anchor: `{"kind":"cell","range":"B4","sheet":"Q3"}`. */
export function cellKey(sheet: string, range: string) {
  return JSON.stringify({ kind: "cell", range, sheet });
}

/** The sheet and range an anchor names; null when it is not a cell anchor. */
export function cellOfKey(key: string): { sheet: string; range: string } | null {
  try {
    const a = JSON.parse(key);
    return a && a.kind === "cell" && typeof a.sheet === "string" && typeof a.range === "string" ? { sheet: a.sheet, range: a.range } : null;
  } catch {
    return null;
  }
}

/** What the foot says this file has that the grid does not draw. */
export function notesOf(outline: Pick<Outline, "nodes">) {
  const out: string[] = [];
  for (const s of outline.nodes.filter((n) => n.type === "sheet")) {
    const p = (s.props || {}) as { charts?: { title?: string }[]; pivots?: number };
    for (const c of p.charts || []) out.push(`${s.name}: the chart "${c.title || "untitled"}" is not drawn here (it stays in the file).`);
    if (p.pivots) out.push(`${s.name}: ${p.pivots} pivot table${p.pivots > 1 ? "s are" : " is"} not drawn here (kept in the file).`);
  }
  return out;
}

/**
 * His edits saved, without the DOM. `grid.snapshot()` is the grid now; `body(before, now)` is
 * what to write for the edits between two snapshots (an xlsx's bytes, a csv's text), null when
 * nothing changed; `write(body, ifMatch)` is the kit's save; `later(fn, ms)` schedules and gives
 * its cancel; `onState` is told each state.
 */
export function saver({ grid, etag, body, write, later, onState }: {
  grid: Pick<Grid, "snapshot">;
  etag: string;
  body(before: UWorkbook, now: UWorkbook): Promise<string | Uint8Array | null>;
  write(body: string | Uint8Array, ifMatch: string): Promise<Saved>;
  later(fn: () => void, ms: number): () => void;
  onState(s: SaveState): void;
}) {
  let saved = grid.snapshot(); // the grid's snapshot the file was last written from
  let ifMatch = etag; // the version he read or saved last: a conflict keeps it
  let state: SaveState = { kind: "idle" };
  let cancel: (() => void) | null = null;
  let inFlight: Promise<void> | null = null;
  let dirty = false;
  const set = (s: SaveState) => {
    state = s;
    onState(s);
  };
  const schedule = (ms: number) => {
    cancel?.();
    cancel = later(save, ms);
  };

  function changed() {
    dirty = true;
    if (state.kind !== "saving" && state.kind !== "conflict") set({ kind: "waiting" });
    schedule(SAVE_AFTER_MS);
  }

  async function save(): Promise<void> {
    cancel?.();
    cancel = null;
    if (inFlight) return inFlight;
    inFlight = (async () => {
      const now = grid.snapshot();
      const next = await body(saved, now);
      dirty = false;
      if (next === null) {
        if (state.kind !== "conflict") set({ kind: "idle" });
        return;
      }
      const before = saved;
      saved = now;
      if (state.kind !== "conflict") set({ kind: "saving" });
      try {
        const r = await write(next, ifMatch);
        if (r.outcome === "saved") {
          ifMatch = r.etag;
          set({ kind: "saved" });
        } else if (r.outcome === "conflict") set({ kind: "conflict" });
        else {
          saved = before;
          set({ kind: "failed", why: `the file is over ${Math.round(r.maxBytes / (1024 * 1024))} MB` });
        }
      } catch (e) {
        // the kit tried 3 times with the same seq (or the hub refused): his edits stay, a later try
        saved = before;
        dirty = true;
        set({ kind: "failed", why: e instanceof Error ? e.message : String(e) });
        schedule(RETRY_MS);
      }
    })();
    try {
      await inFlight;
    } finally {
      inFlight = null;
    }
    if (dirty && state.kind !== "failed") changed();
  }

  return {
    get state() {
      return state;
    },
    get dirty() {
      return dirty;
    },
    changed,
    save,
  };
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string | null) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  return e;
}

/** What the grid needs of the view. */
export type Host = {
  /** the kit's save */
  write(body: string | Uint8Array, ifMatch: string): Promise<Saved>;
  /** his selection (a range), its words, where it is */
  select(anchor: string | null, text: string | null, at: DOMRect | null): void;
  /** light or dark now */
  dark(): boolean;
  onDark(fn: (dark: boolean) => void): void;
};

export type Drawn = {
  notes: string[];
  place(): GridPlace;
  restore(place: unknown): void;
  onChange(fn: () => void): void;
  find(anchor: string): Element | null;
  /** the edits not saved yet, saved now (the tab is going) */
  flush(): Promise<void>;
};

/** The workbook of a file: an xlsx's bytes, or a csv's text as one sheet. */
export async function bookOf(name: string, file: string | Uint8Array): Promise<{ book: Book; csv: string | null }> {
  if (/\.csv$/i.test(name)) {
    const text = typeof file === "string" ? file : new TextDecoder("utf-8").decode(file);
    const sep = sepOf(text);
    const rows = parseCsv(text, sep).map((r) => r.map(cellOf));
    return { book: await sheets.create({ sheet: name.replace(/\.csv$/i, "").slice(0, 31) || "Sheet1", rows }), csv: sep };
  }
  if (typeof file === "string") throw new Error("it came as text, not as an Excel file");
  return { book: await sheets.open(file), csv: null };
}

export async function draw(root: HTMLElement, name: string, file: string | Uint8Array, opts: { etag: string; writable: boolean }, host: Host): Promise<Drawn> {
  const [{ book, csv }, ui] = await Promise.all([bookOf(name, file), import("./office-sheet.js") as Promise<GridModule>]);
  root.classList.add("g-sheet");
  const box = el("div", "g-univer");
  root.append(box);

  // the grid needs its size: mounted once the view put the stage in the page
  await new Promise<void>((ok) => {
    const wait = () => (box.isConnected ? ok() : requestAnimationFrame(wait));
    wait();
  });
  const grid = ui.mountGrid(box, book.univer(), { dark: host.dark(), readOnly: !opts.writable });
  host.onDark((on) => grid.dark(on));

  // the bytes (an xlsx: the grid's edits on the file) or the text (a csv) of a save
  const body = async (before: UWorkbook, now: UWorkbook) => {
    if (csv !== null) return csvOf(now, csv) === csvOf(before, csv) ? null : csvOf(now, csv);
    const edits = editsBetween(before, now);
    if (edits.length === 0) return null;
    book.apply(edits);
    return book.save();
  };
  const save = opts.writable
    ? saver({
        grid,
        etag: opts.etag,
        body,
        write: host.write,
        later: (fn, ms) => {
          const t = setTimeout(fn, ms);
          return () => clearTimeout(t);
        },
        onState: (s) => (root.dataset.save = s.kind),
      })
    : null;
  if (save) grid.onChange(() => save.changed());

  // his selection: a range, its words, the pill by it
  const changed = new Set<() => void>();
  box.addEventListener("pointerup", () => setTimeout(say, 0));
  box.addEventListener("keyup", () => setTimeout(say, 0));
  function say() {
    const p = grid.place();
    changed.forEach((fn) => fn());
    if (!p.range) return host.select(null, null, null);
    host.select(cellKey(p.sheet, p.range), grid.text() || p.range, grid.rect(p.sheet, p.range));
  }

  // the marks of his notes: a box over their cells, kept there as the grid moves
  const marks = new Map<string, HTMLDivElement>();
  function moveMarks() {
    for (const [key, m] of marks) {
      if (!m.isConnected) {
        marks.delete(key);
        continue;
      }
      const c = cellOfKey(key)!;
      const r = grid.rect(c.sheet, c.range);
      m.hidden = !r;
      if (r) Object.assign(m.style, { left: `${r.left}px`, top: `${r.top}px`, width: `${r.width}px`, height: `${r.height}px` });
    }
  }
  grid.onMove(moveMarks);

  return {
    notes: notesOf(book.inspect({ maxCells: 0 })),
    place: () => grid.place(),
    restore(p) {
      const at = p as GridPlace | null;
      if (at && typeof at.sheet === "string") grid.restore(at);
    },
    onChange: (fn) => changed.add(fn),
    find(anchor) {
      const c = cellOfKey(anchor);
      if (!c || !book.sheetNames.includes(c.sheet)) return null;
      let m = marks.get(anchor);
      if (!m) {
        m = el("div", "g-cell-mark");
        document.body.append(m);
        marks.set(anchor, m);
      }
      moveMarks();
      return m;
    },
    flush: async () => {
      if (save && save.dirty) await save.save();
    },
  };
}
