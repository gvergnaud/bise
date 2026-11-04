// /debug's words and rows (the TUI's rust/tui/src/logview.rs, designer m_17266 and
// debug-views v3): the role column's label and colour kind, a header's size, a fold's
// '▸ 34 lines · 2.1 KB', the rules, the pages kept oldest first, the hub's hits stepped, and
// the completion tab's rows. Owners: amb-web, debug-cmd. Pure.
import type {
  CompBlock,
  CompRow,
  Completion,
  LogItem,
  LogRole,
} from "@/proto";

/** A body folds past this many lines (designer m_17266: past 6 lines). */
export const FOLD_LINES = 6;

/** The colour a row's label and rail take (logview.rs kind_color). */
export type Kind =
  | "you"
  | "agent"
  | "thinking"
  | "call"
  | "result"
  | "failed"
  | "message"
  | "injected"
  | "system"
  | "summary"
  | "error"
  | "event";

export function kindOf(it: LogItem): Kind {
  switch (it.role) {
    case "you":
      return "you";
    case "assistant":
      return "agent";
    case "thinking":
      return "thinking";
    case "call":
      return "call";
    case "result":
      return it.ok === false ? "failed" : "result";
    case "message":
      return "message";
    case "injected":
      return "injected";
    case "system":
    case "tools":
      return "system";
    case "summary":
      return "summary";
    case "error":
      return "error";
    case "event":
    case "unknown":
      return "event";
  }
}

/** The assistant's word: the agent's name when it fits (7 chars), else 'agent'. */
const agentWord = (name: string): string => (name.length <= 7 ? name : "agent");

const CALLS: Readonly<Record<string, string>> = {
  bash: "$ bash",
  run_typescript: "⌁ ts",
  edit: "± edit",
  write_file: "± write",
  apply_patch: "± patch",
  read_file: "⇄ read",
};

/** The role column's words (logview.rs role_label). */
export function labelOf(it: LogItem, agent: string): string {
  const role: LogRole = it.role;
  switch (role) {
    case "you":
      return "› you";
    case "assistant":
      return `● ${agentWord(agent)}`;
    case "thinking":
      return "∴ thinking";
    case "call":
      return CALLS[it.tool ?? ""] ?? it.tool ?? "call";
    case "result":
      return it.ok === false ? "✗ result" : "└ result";
    case "message":
      return "@ message";
    case "injected":
      return "⊕ bise";
    case "system":
      return "§ system";
    case "tools":
      return "§ tools";
    case "summary":
      return "≡ summary";
    case "error":
      return "■ error";
    case "event":
    case "unknown":
      return "· event";
  }
}

/** The TUI's short_bytes: '512 B', '2.1 KB', '1.4 MB'. */
export function shortBytes(n: number): string {
  if (n < 1024) return `${String(n)} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

/** UTF-8 bytes of a text, as the TUI counts them. */
export function bytesOf(text: string): number {
  let n = 0;
  for (const ch of text) {
    const c = ch.codePointAt(0) ?? 0;
    n += c < 0x80 ? 1 : c < 0x800 ? 2 : c < 0x10000 ? 3 : 4;
  }
  return n;
}

/** A header's right column: its body's size, none without a body. */
export function sizeOf(it: LogItem): string {
  const t = it.body?.text ?? "";
  return t === "" ? "" : shortBytes(bytesOf(t));
}

export const linesOf = (text: string): number =>
  Math.max(1, text.replace(/\n+$/u, "").split("\n").length);

/** Whether a body folds (past FOLD_LINES lines). */
export const folds = (it: LogItem): boolean =>
  it.body !== undefined && linesOf(it.body.text) > FOLD_LINES;

/** A fold's words (logview.rs entry_rows): '▸ 34 lines · 2.1 KB'. */
export function foldLabel(it: LogItem): string {
  const t = it.body?.text ?? "";
  return `▸ ${String(linesOf(t))} lines · ${shortBytes(bytesOf(t))}`;
}

/** A rule's kind: a compaction draws green, a turn faint. */
export const ruleKind = (rule: string): "compact" | "turn" =>
  rule.includes("compacted") ? "compact" : "turn";

/** A header's time: HH:MM:SS local, none without one. */
export function timeOf(atMs: number | undefined, tz?: string): string {
  if (atMs === undefined) return "";
  return new Date(atMs).toLocaleTimeString("en-GB", {
    hour12: false,
    ...(tz === undefined ? {} : { timeZone: tz }),
  });
}

/**
 * Bodies drawn as the thread's prose (the rest in mono, in the code box). His rows are text as
 * sent (file-mention-wire): markdown would hide the image markers and tags the model reads.
 */
export const prose = (it: LogItem): boolean =>
  it.body?.kind === "md" &&
  (it.role === "assistant" ||
    it.role === "thinking" ||
    it.role === "summary" ||
    it.role === "message" ||
    it.role === "injected");

/** A code body's text: JSON pretty, the rest as written. */
export function codeText(it: LogItem): { text: string; lang: string } {
  const b = it.body;
  if (b === undefined) return { text: "", lang: "text" };
  const looksJson =
    b.lang === "json" || (b.lang === undefined && b.kind !== "md" && /^\s*[[{]/u.test(b.text));
  if (looksJson && b.cut !== true) {
    try {
      return {
        text: JSON.stringify(JSON.parse(b.text), null, 2),
        lang: "json",
      };
    } catch {
      // not JSON after all: as written
    }
  }
  return { text: b.text, lang: b.lang ?? "text" };
}

/** An edit as the thread shows it (designer m_17531): its file, '+1 −1', and − / + lines. */
export interface EditDiff {
  path: string;
  add: number;
  rm: number;
  lines: { sign: "+" | "-"; text: string }[];
}

const str = (v: unknown): string | undefined =>
  typeof v === "string" ? v : undefined;
const NL = "\u000A";
/** A text's lines, a last newline ending the last one (not a blank line of its own). */
const lines = (t: string): string[] =>
  t === "" ? [] : (t.endsWith(NL) ? t.slice(0, -1) : t).split(NL);

/** An edit or write call's args as a diff (the old lines out, the new ones in); null for the rest. */
export function editDiff(it: LogItem): EditDiff | null {
  if (it.role !== "call" || it.body === undefined || it.body.cut === true)
    return null;
  if (it.tool !== "edit" && it.tool !== "write_file") return null;
  let args: Record<string, unknown>;
  try {
    const v: unknown = JSON.parse(it.body.text);
    if (typeof v !== "object" || v === null || Array.isArray(v)) return null;
    args = v as Record<string, unknown>;
  } catch {
    return null;
  }
  const path = str(args.path) ?? str(args.file_path);
  if (path === undefined) return null;
  const old = it.tool === "edit" ? (str(args.old) ?? str(args.old_string)) : "";
  const neu =
    it.tool === "edit"
      ? (str(args.new) ?? str(args.new_string))
      : str(args.content);
  if (old === undefined || neu === undefined) return null;
  const rm = lines(old);
  const add = lines(neu);
  return {
    path,
    add: add.length,
    rm: rm.length,
    lines: [
      ...rm.map((text) => ({ sign: "-" as const, text })),
      ...add.map((text) => ({ sign: "+" as const, text })),
    ],
  };
}

/** Where a needle sits in a text, case-insensitive: each match's start (the find's marks). */
export function matchesIn(text: string, needle: string): number[] {
  const n = needle.trim().toLowerCase();
  if (n === "") return [];
  const t = text.toLowerCase();
  const out: number[] = [];
  for (let i = t.indexOf(n); i !== -1; i = t.indexOf(n, i + n.length))
    out.push(i);
  return out;
}

/** The rows so far, oldest first: an older page goes before them (it holds seqs below theirs). */
export function withOlder(
  rows: readonly LogItem[],
  older: readonly LogItem[],
): LogItem[] {
  const first = rows[0]?.pos;
  return [
    ...(first === undefined ? older : older.filter((o) => o.pos < first)),
    ...rows,
  ];
}

/** The hit to go to from `at` by `by` (n, N), wrapping; -1 without hits. */
export const stepHit = (at: number, by: number, of: number): number =>
  of === 0 ? -1 : (((at + by) % of) + of) % of;

/** The find's count at the key line's right: '3 of 12', 'no match'. */
export const findCount = (at: number, of: number): string =>
  of === 0 ? "no match" : `${String(at + 1)} of ${String(of)}`;

/** The filter's kinds, in the TUI's order. */
export const KINDS: readonly Kind[] = [
  "you",
  "agent",
  "thinking",
  "call",
  "result",
  "failed",
  "message",
  "injected",
  "system",
  "summary",
  "error",
  "event",
];

/** The kinds the rows hold, in KINDS' order (the filter offers these). */
export const kindsIn = (rows: readonly LogItem[]): Kind[] =>
  KINDS.filter((k) =>
    rows.some((r) => r.rule === undefined && kindOf(r) === k),
  );

/** A row shows under a filter (rules always show). */
export const shown = (it: LogItem, hidden: ReadonlySet<Kind>): boolean =>
  it.rule !== undefined || !hidden.has(kindOf(it));

// ---- /debug completion (the hub's CompRow, bise_session::completion) ----

/** The TUI's short_num (bise_session::history): 18, 1.2k, 41k, 1.2M. */
export function shortNum(n: number): string {
  if (n < 1000) return String(n);
  if (n < 10_000) return `${(n / 1000).toFixed(1)}k`;
  if (n < 1_000_000) return `${String(Math.floor(n / 1000))}k`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

/** A row's right column: '~1.2k tok' (bytes / 4, the TUI's estimate). */
export const tokensWord = (n: number): string => `~${shortNum(n)} tok`;

/** How a block draws: bash a command box, code highlighted, markdown, text. */
export function compBlockView(b: CompBlock): "cmd" | "code" | "md" | "text" {
  if (b.body.kind === "md") return "md";
  if (b.body.kind === "code" && b.body.lang === "bash") return "cmd";
  if (b.body.kind === "code" && b.body.lang !== undefined) return "code";
  return "text";
}

/** A row folds: the request's head and the system prompt always, the rest past FOLD_LINES. */
export const compFolds = (r: CompRow): boolean =>
  r.role === "request" ||
  r.role === "system" ||
  r.blocks.reduce((n, b) => n + linesOf(b.body.text), 0) > FOLD_LINES;

/** The `n` of every row that holds a needle (any case): its role, head, labels, texts. */
export function compHits(rows: readonly CompRow[], needle: string): number[] {
  const n = needle.trim().toLowerCase();
  if (n === "") return [];
  const has = (s: string | undefined): boolean =>
    s !== undefined && s.toLowerCase().includes(n);
  return rows
    .filter(
      (r) =>
        has(r.role) ||
        has(r.head) ||
        r.blocks.some((b) => has(b.label) || has(b.body.text)),
    )
    .map((r) => r.n);
}

/** The request's line: 'request 41 · 8 requests kept · anthropic · claude-x · 14:02:01 · 41.2 KB'. */
export function compTitle(c: Completion, tz?: string): string {
  const ready = c.slots.filter((s) => s.writing !== true);
  const as = c.shown_as === undefined ? "" : ` · as ${c.shown_as} would get it`;
  return [
    `request ${String(c.slot.req)}`,
    `${String(ready.length)} ${ready.length === 1 ? "request" : "requests"} kept`,
    c.family,
    c.slot.model,
    timeOf(c.slot.at_ms, tz),
    shortBytes(c.bytes),
  ].join(" · ") + as;
}

/** A row's role word: a message with no text of its own (only calls, or only results) goes by
 *  its first block's ('tool_use · bash', 'result'), never by its provider role (designer m_37596). */
export const compRole = (r: CompRow): string =>
  r.n > 0 && r.blocks.length > 0 && r.blocks.every((b) => b.label !== undefined)
    ? (r.blocks[0]?.label ?? r.role)
    : r.role;
