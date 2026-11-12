/// <reference lib="dom" />
// Sheets' grid, office-sheet.js in its view's folder (page xlsx-univer,
// architect m_37322, m_60401): Univer (Apache-2.0, open-source packages
// only, no Univer Pro) on a snapshot the SDK made
// (office/sdk/src/sheets/univer.ts). Built by office/sdk/build.ts with its
// lockfile, never committed (scripts/bins.sh office-sheet places it), loaded
// only by Sheets' book.js. Its stylesheet is in it as text, put in the
// page once (one file to serve under the view's CSP).
//
// What it gives book.js: the grid in a container, the grid's snapshot
// now (Univer's own save, for editsBetween), a call after each change the
// user made (a typed value, a resize, a paste, an undo), his selection,
// the light or dark look, read only when the document is (an older
// version, no write grant). The ribbon, the context menus and the sheet
// menus are off: Sheets edits values, formulas and sizes, no more.

import { CommandType, LocaleType, LogLevel, mergeLocales, Univer, type IWorkbookData } from "@univerjs/core";
import { FUniver } from "@univerjs/core/facade";
import { defaultTheme } from "@univerjs/themes";
import { UniverSheetsCorePreset } from "@univerjs/preset-sheets-core";
import enUS from "@univerjs/preset-sheets-core/locales/en-US";
// @ts-expect-error esbuild's text loader (build.ts): the stylesheet as a string
import css from "@univerjs/preset-sheets-core/lib/index.css";
import type { UWorkbook } from "../../../office/sdk/src/sheets/univer.ts";

export type GridPlace = { sheet: string; range: string | null };

export type Grid = {
  /** the grid as it is now, as Univer saves it */
  snapshot(): UWorkbook;
  /** called after each change the user made (never for opening or a selection) */
  onChange(fn: () => void): void;
  /** the sheet shown and the selected range ("B4:D9"), or null */
  place(): GridPlace;
  /** show that sheet and select that range again */
  restore(p: GridPlace): void;
  /** where a range of the sheet shown is, in the page's pixels (null: another sheet, or off screen) */
  rect(sheet: string, range: string): DOMRect | null;
  /** the selection's text, as the grid shows it, cells joined by spaces */
  text(): string;
  /** called when the grid scrolls, zooms, or shows another sheet (the marks follow) */
  onMove(fn: () => void): void;
  dark(on: boolean): void;
  dispose(): void;
};

// bise's look on Univer's scales (kit/tokens.css; designer signs it): a selection, a picked
// tab, the formula bar's focus are ink, never the accent (the accent is only what is his);
// the grays are the kit's warm ones. Univer's dark mode turns the same scales around.
const ink = { 50: "#f6f3ed", 100: "#ece7de", 200: "#d9d2c5", 300: "#b5ad9f", 400: "#8a8174", 500: "#5a5349", 600: "#3a352f", 700: "#2a2622", 800: "#1d1a17", 900: "#171513" };
const theme = {
  ...defaultTheme,
  primary: ink,
  gray: { 0: "#ffffff", 50: "#faf7f0", 100: "#f2ede2", 200: "#e6e0d4", 300: "#b5ad9f", 400: "#8a8174", 500: "#5a5349", 600: "#3a352f", 700: "#2a2622", 800: "#1f1c19", 900: "#171513", 1000: "#000000" },
} as typeof defaultTheme;

// the app's sans (kit/tokens.css --font-ui) on the canvas's chrome: the column letters and row
// numbers (the formula bar and the tabs are DOM: viewer.css); never on the cells
const FONT = '-apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif';
// the DS overlay scrollbar's look: a 4 px thumb in text-3, no track
const scrollConfig = { barSize: 4, thumbMargin: 2, barBorder: 0, trackBackgroundColor: "rgba(0,0,0,0)", trackBorderColor: "rgba(0,0,0,0)", thumbBackgroundColor: "rgba(138,129,116,0.6)", thumbHoverBackgroundColor: "rgba(138,129,116,0.8)", thumbActiveBackgroundColor: "rgba(138,129,116,0.9)", hideTrackWhenUnscrollable: true };

let styled = false;

/** Univer's stylesheet in the page, once. */
function style(): void {
  if (styled) return;
  styled = true;
  const s = document.createElement("style");
  s.dataset.univer = "";
  s.textContent = css as string;
  document.head.append(s);
}

/** The grid of `data` in `el` (which must have a size). */
export function mountGrid(el: HTMLElement, data: UWorkbook, opts: { dark: boolean; readOnly?: boolean }): Grid {
  style();
  const univer = new Univer({
    theme,
    darkMode: opts.dark,
    locale: LocaleType.EN_US,
    locales: { [LocaleType.EN_US]: mergeLocales(enUS) },
    logLevel: LogLevel.ERROR,
  });
  const preset = UniverSheetsCorePreset({
    container: el,
    header: true,
    toolbar: false,
    contextMenu: false,
    formulaBar: true,
    sheets: { scrollConfig },
    footer: { sheetBar: true, statisticBar: false, menus: false, zoomSlider: false } as never,
  } as never);
  for (const p of preset.plugins) {
    if (Array.isArray(p)) univer.registerPlugin(p[0], p[1]);
    else univer.registerPlugin(p);
  }
  const api = FUniver.newAPI(univer);
  // the cells keep the file's own font and sizes (designer m_37490): only the chrome is ours
  const book = api.createWorkbook({ ...(data as unknown as Partial<IWorkbookData>), locale: LocaleType.EN_US });
  // a document he can't save (an older version, no write grant): Univer's viewer mode, no edit
  if (opts.readOnly) void book.getWorkbookPermission().setReadOnly();
  // the headers' font: once the grid's render unit exists (it may come a frame later); a
  // header in Univer's face is a look, never a reason to fail the grid
  const header = { headerStyle: { fontFamily: FONT, fontSize: 11 } } as never;
  const headers = (tries: number): void => {
    try {
      for (const ws of book.getSheets()) {
        ws.customizeColumnHeader(header);
        ws.customizeRowHeader(header);
      }
    } catch {
      if (tries > 0) requestAnimationFrame(() => headers(tries - 1));
    }
  };
  headers(30);

  const listeners: (() => void)[] = [];
  const movers: (() => void)[] = [];
  const moved = () => {
    for (const fn of movers) fn();
  };
  // a change is a mutation after he touched the grid (a key, a pointer, a
  // paste): opening, its commands and the formula engine's first pass are not
  let his = false;
  const touched = () => {
    his = true;
  };
  for (const t of ["pointerdown", "keydown", "paste", "drop"]) el.addEventListener(t, touched, { capture: true });
  // the look's own mutations (the gridlines' color) are never his change
  let quiet = false;
  // the gridlines in dark: the kit's rule color (tokens.css --line, the warm grey of the panel's
  // own rules; designer m_37891), never Univer's olive; in light Univer's own (signed as is)
  const lines = (dark: boolean): void => {
    const rule = dark ? getComputedStyle(document.documentElement).getPropertyValue("--line").trim() : "";
    quiet = true;
    try {
      for (const ws of book.getSheets()) ws.setGridLinesColor((rule || undefined) as string);
    } catch {
      // a look, never a reason to fail the grid
    } finally {
      quiet = false;
    }
  };
  const subs = [
    api.addEvent(api.Event.CommandExecuted, (c) => {
      if (c.type === CommandType.MUTATION && his && !quiet) for (const fn of listeners) fn();
    }),
    api.addEvent(api.Event.Scroll, moved),
    api.addEvent(api.Event.ActiveSheetChanged, moved),
  ];
  lines(opts.dark);

  return {
    snapshot: () => book.save() as unknown as UWorkbook,
    onChange: (fn) => {
      listeners.push(fn);
    },
    place() {
      const ws = book.getActiveSheet();
      const r = ws.getSelection()?.getActiveRange();
      return { sheet: ws.getSheetName(), range: r ? r.getA1Notation() : null };
    },
    restore(p) {
      const ws = book.getSheetByName(p.sheet);
      if (!ws) return;
      book.setActiveSheet(ws);
      if (p.range) ws.setActiveRange(ws.getRange(p.range));
    },
    rect(sheet, range) {
      const ws = book.getActiveSheet();
      if (ws.getSheetName() !== sheet) return null;
      try {
        // Univer's cell rects are in the sheet's own pixels (the headers in, the scroll and the
        // zoom out), from the grid canvas's corner: never the container's, which has the
        // formula bar above the canvas (the bar is a render canvas too: the grid's is the largest)
        const area = (r: DOMRect) => r.width * r.height;
        const boxes = Array.from(el.querySelectorAll<HTMLCanvasElement>('canvas[data-u-comp="render-canvas"]'), (c) => c.getBoundingClientRect());
        const box = boxes.reduce<DOMRect | null>((m, r) => (!m || area(r) > area(m) ? r : m), null);
        if (!box) return null;
        const [a, b = a] = range.split(":");
        const r0 = ws.getRange(a).getCellRect();
        const r1 = ws.getRange(b).getCellRect();
        const head = ws.getRange("A1").getCellRect();
        const s = ws.getScrollState();
        const first = ws.getRange(s.sheetViewStartRow, s.sheetViewStartColumn).getCellRect();
        const zoom = ws.getZoom() || 1;
        const x = (v: number) => box.left + (head.left + (v - first.left - s.offsetX) * zoom);
        const y = (v: number) => box.top + (head.top + (v - first.top - s.offsetY) * zoom);
        const left = x(Math.min(r0.left, r1.left));
        const top = y(Math.min(r0.top, r1.top));
        return new DOMRect(left, top, x(Math.max(r0.right, r1.right)) - left, y(Math.max(r0.bottom, r1.bottom)) - top);
      } catch {
        return null;
      }
    },
    text() {
      const r = book.getActiveSheet().getSelection()?.getActiveRange();
      return r ? r.getDisplayValues().flat().filter(Boolean).join(" ") : "";
    },
    onMove: (fn) => {
      movers.push(fn);
    },
    dark: (on) => {
      api.toggleDarkMode(on);
      lines(on);
    },
    dispose() {
      for (const s of subs) s.dispose();
      univer.dispose();
    },
  };
}
