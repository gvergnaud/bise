// Uploads' pure parts (sw/upload.ts drives them): the file choosers a tab
// opened, the files an act names, the summary line, and the candidates of
// a `no_file_input`. Pure.

/**
 * A file chooser the page opened while bise was attached: intercepted,
 * never a native dialog. `input`: an <input type=file> (its node);
 * `fsa`: a parked `showOpenFilePicker()` call (bise's shim, `fsa` its
 * number in the page); `picker`: one bise can't feed (no node, no shim).
 */
export type Chooser = { id: number; kind: "input" | "fsa" | "picker"; backendNodeId?: number; multiple: boolean; fsa?: number; used?: boolean };

/** At most this many choosers are kept per tab. */
export const KEEP = 5;

/** The list with `c` added, the oldest dropped past KEEP. */
export function remember(list: Chooser[], c: Chooser): Chooser[] {
  return [...list, c].slice(-KEEP);
}

/** The first chooser opened after `from` that bise can feed and nobody used yet. */
export function nextChooser(list: Chooser[], from: number): Chooser | null {
  return list.find((c) => c.id > from && c.kind !== "picker" && !c.used) ?? null;
}

/** Did a chooser bise can't feed open after `from`? (the error says so) */
export function sawPicker(list: Chooser[], from: number): boolean {
  return list.some((c) => c.id > from && c.kind === "picker");
}

/** An act's `files`: a list of absolute paths, or why not (the broker checked them already). */
export function filesOf(files: unknown): string[] | string {
  const list = typeof files === "string" ? [files] : files;
  if (!Array.isArray(list) || list.length === 0) return "set_input_files needs files: a list of absolute paths";
  if (!list.every((f) => typeof f === "string" && f.startsWith("/"))) return "files are absolute paths (strings starting with /)";
  return list as string[];
}

/** A path's file name. */
export const baseName = (p: string) => p.slice(p.lastIndexOf("/") + 1);

/** `a.pdf, b.pdf, c.pdf` or `a.pdf, b.pdf and 3 more`: at most 3 names. */
export function namesLine(names: string[]): string {
  if (names.length <= 3) return names.join(", ");
  return `${names.slice(0, 2).join(", ")} and ${names.length - 2} more`;
}

/** The summary line: `attached a.pdf, b.pdf to "Upload payslip" · cafpi.fr`. */
export function uploadSummary(names: string[], target: string, host: string | null | undefined): string {
  return `attached ${namesLine(names)}${target ? ` to ${target}` : ""}${host ? ` · ${host}` : ""}`;
}

/** Words of a button or link that may open a file chooser (English, French). */
export const UPLOADISH = /upload|attach|browse|choose|select file|file|drop|joindre|t[ée]l[ée]charger|t[ée]l[ée]verser|parcourir|ajouter|d[ée]poser|fichier|document|justificatif|pi[eè]ce/i;

/** One snapshot line, as candidates carry them. */
type Line = { role: string; name?: string; line?: string };

/**
 * A `no_file_input`'s candidates: the page's file inputs (as the page
 * names them; hidden ones say so), then the buttons and links whose name
 * sounds like an upload; at most 10.
 */
export function uploadCandidates(inputs: { name: string; hidden: boolean }[], entries: Line[]): string[] {
  const own = inputs.map((i) => `- file input "${i.name}"${i.hidden ? " (hidden: target the button next to it, or this input by its label)" : ""}`);
  const near = entries.filter((e) => ["button", "link", "region", "generic"].includes(e.role) && e.name && UPLOADISH.test(e.name)).map((e) => e.line ?? "");
  return [...own, ...near].slice(0, 10);
}
