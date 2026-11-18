// Code's pure half (plugins/code): a file's text as lines, which lines a scroll shows, which
// grammars a language needs and in what order, and prism's tokens as runs of text with bise's
// syntax classes (kit/view/code.ts ROLES, the window's code blocks' table). No DOM, no prism: the
// view (code.ts) gives it prism's tokens. Big files (architect m_66499 (4)): only the lines on
// screen (and a margin) are drawn, and colored by blocks of BLOCK lines, each tokenized alone
// after LEAD lines of the block before it (a comment or a string opened there still reads).

import { roleOf } from "./~kit/view/code.js";

/** One line's height, px (designer m_66504: 13 px mono on a 20 px line). */
export const LINE = 20;
/** Lines drawn past the screen's top and bottom. */
export const MARGIN = 40;
/** Lines colored together (one prism run). */
export const BLOCK = 400;
/** Lines of the block before tokenized first, then dropped (their context only). */
export const LEAD = 60;
/** Past this many bytes a file is drawn plain, never colored (designer m_66504: ~1 MB). */
export const PLAIN_OVER = 1024 * 1024;
/** A ranged read's length. */
export const CHUNK = 4 * 1024 * 1024;
/** A tab's width in columns (tab-size in code.css). */
const TAB = 4;

/** The vendored grammars (prism/<id>.js, prismjs 1.30.0) and the ones each needs first. */
export const GRAMMARS: Readonly<Record<string, readonly string[]>> = {
  clike: [], markup: [], css: [], javascript: ["clike"], typescript: ["javascript"],
  jsx: ["markup", "javascript"], tsx: ["jsx", "typescript"], python: [], rust: [], go: ["clike"],
  swift: [], kotlin: ["clike"], java: ["clike"], c: ["clike"], cpp: ["c"], csharp: ["clike"],
  ruby: ["clike"], php: ["markup-templating"], "markup-templating": ["markup"], bash: [], sql: [],
  json: [], yaml: [], toml: [], scss: ["css"], markdown: ["markup"], docker: [], makefile: [],
  graphql: [], lua: [], ini: [], objectivec: ["c"],
};

/** The grammar files to load for `lang`, each after the ones it needs, once; none for an unknown one. Pure. */
export function loadOrder(lang: string | undefined): string[] {
  const out: string[] = [];
  const add = (l: string) => {
    if (out.includes(l) || GRAMMARS[l] === undefined) return;
    for (const d of GRAMMARS[l]!) add(d);
    out.push(l);
  };
  if (lang !== undefined) add(lang);
  return out;
}

/** A text's lines (a final newline makes no last empty line; CRLF read as LF). Empty: none. Pure. */
export function linesOf(text: string): string[] {
  if (text === "") return [];
  const lines = text.split(/\r?\n/);
  if (lines.length > 1 && lines[lines.length - 1] === "") lines.pop();
  return lines;
}

/** A line's width in columns, a tab to the next multiple of TAB. Pure. */
export function columns(line: string): number {
  let w = 0;
  for (const ch of line) w = ch === "\t" ? (Math.floor(w / TAB) + 1) * TAB : w + 1;
  return w;
}

/** The widest line's columns. Pure. */
export function widest(lines: readonly string[]): number {
  let w = 0;
  for (const l of lines) if (l.length > w || l.includes("\t")) w = Math.max(w, columns(l));
  return w;
}

/** The lines a scroll shows, with MARGIN around: [from, to). Pure. */
export function shown(top: number, height: number, count: number): { from: number; to: number } {
  const first = Math.max(0, Math.floor(top / LINE) - MARGIN);
  const last = Math.min(count, Math.ceil((top + height) / LINE) + MARGIN);
  return { from: first, to: Math.max(first, last) };
}

/** Block `b`'s text to tokenize: its LEAD lines before, then its own; `skip` the lead's count. Pure. */
export function blockText(lines: readonly string[], b: number): { text: string; skip: number } {
  const start = b * BLOCK;
  const lead = Math.max(0, start - LEAD);
  return { text: lines.slice(lead, start + BLOCK).join("\n"), skip: start - lead };
}

/** A prism token as prism gives it (prism-core's Token, or a bare string). */
export type Tok = string | { readonly type: string; readonly content: Tok | readonly Tok[]; readonly alias?: string | readonly string[] };

/** A run of a line: its text and its token types, outer first ([] for plain). */
export type Run = { readonly text: string; readonly types: readonly string[] };

/**
 * prism's token stream as lines of runs, a token's types (its type, then its aliases) inside its
 * parents', a newline inside a token splitting its run (prism-react-renderer's normalizeTokens). Pure.
 */
export function runsOf(tokens: readonly Tok[]): Run[][] {
  const lines: Run[][] = [[]];
  const walk = (t: Tok | readonly Tok[], types: readonly string[]) => {
    if (typeof t === "string") {
      const parts = t.split("\n");
      parts.forEach((p, i) => {
        if (i > 0) lines.push([]);
        if (p !== "") lines[lines.length - 1]!.push({ text: p, types });
      });
      return;
    }
    if (Array.isArray(t)) {
      for (const x of t as readonly Tok[]) walk(x, types);
      return;
    }
    const tok = t as Exclude<Tok, string>;
    const alias = tok.alias === undefined ? [] : typeof tok.alias === "string" ? [tok.alias] : [...tok.alias];
    walk(tok.content, [...types, tok.type, ...alias]);
  };
  for (const t of tokens) walk(t, []);
  return lines;
}

/** A run's classes (code.css): its role's hue, and b/n/i/s for bold, normal, italic, struck. Pure. */
export function classesOf(types: readonly string[], text: string): string {
  if (types.length === 0) return "";
  const r = roleOf(types, text);
  return [
    r.hue === undefined ? "" : `syn-${r.hue}`,
    r.bold === true ? "syn-b" : r.normal === true ? "syn-n" : "",
    r.italic === true ? "syn-i" : "",
    r.strike === true ? "syn-s" : "",
  ].filter((c) => c !== "").join(" ");
}

/** A size as the words say it ('1 MB'). Pure. */
export function sizeWords(bytes: number): string {
  return bytes >= 1024 * 1024 ? `${Math.round(bytes / (1024 * 1024))} MB` : `${Math.round(bytes / 1024)} KB`;
}
