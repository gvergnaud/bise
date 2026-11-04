import { describe, expect, it } from "vitest";
import type { LogItem } from "@/proto";
import {
  codeText,
  prose,
  editDiff,
  findCount,
  stepHit,
  compRole,
  foldLabel,
  folds,
  kindOf,
  kindsIn,
  labelOf,
  matchesIn,
  ruleKind,
  shortBytes,
  shown,
  sizeOf,
  timeOf,
  withOlder,
} from "./model";

const item = (
  seq: number,
  role: LogItem["role"],
  over: Partial<LogItem> = {},
): LogItem => ({ seq, pos: seq, role, head: `row ${String(seq)}`, ...over });

describe("log model: edits and find marks (designer m_17531)", () => {
  it("draws an edit's args as − / + lines with its counts", () => {
    const edit = item(1, "call", {
      tool: "edit",
      body: {
        kind: "code",
        lang: "json",
        text: JSON.stringify({
          file_path: "src/a.rs",
          old_string: "a\u000Ab",
          new_string: "c",
        }),
      },
    });
    expect(editDiff(edit)).toEqual({
      path: "src/a.rs",
      add: 1,
      rm: 2,
      lines: [
        { sign: "-", text: "a" },
        { sign: "-", text: "b" },
        { sign: "+", text: "c" },
      ],
    });
    const write = item(2, "call", {
      tool: "write_file",
      body: {
        kind: "code",
        text: JSON.stringify({ path: "n.md", content: "x\u000Ay\u000A" }),
      },
    });
    expect(editDiff(write)?.lines.map((l) => l.sign)).toEqual(["+", "+"]);
    // not an edit, cut, or not JSON: as written
    expect(editDiff(item(3, "call", { tool: "bash" }))).toBeNull();
    expect(
      editDiff(
        item(4, "call", {
          tool: "edit",
          body: { kind: "code", text: '{"path":"a"', cut: true },
        }),
      ),
    ).toBeNull();
    expect(
      editDiff(
        item(5, "call", { tool: "edit", body: { kind: "code", text: "nope" } }),
      ),
    ).toBeNull();
  });

  it("finds a needle's places, case-insensitive, none for blank", () => {
    expect(matchesIn("Query the query", "query")).toEqual([0, 10]);
    expect(matchesIn("aaaa", "aa")).toEqual([0, 2]);
    expect(matchesIn("abc", "  ")).toEqual([]);
  });
});

describe("log model", () => {
  it("labels rows with the TUI's role column words", () => {
    expect(labelOf(item(1, "you"), "perf")).toBe("› you");
    expect(labelOf(item(1, "assistant"), "perf")).toBe("● perf");
    expect(labelOf(item(1, "assistant"), "checkout-flow")).toBe("● agent");
    expect(labelOf(item(1, "call", { tool: "bash" }), "perf")).toBe("$ bash");
    expect(labelOf(item(1, "call", { tool: "run_typescript" }), "perf")).toBe(
      "⌁ ts",
    );
    expect(labelOf(item(1, "call", { tool: "grep" }), "perf")).toBe("grep");
    expect(labelOf(item(1, "result", { ok: true }), "perf")).toBe("└ result");
    expect(labelOf(item(1, "result", { ok: false }), "perf")).toBe("✗ result");
    expect(labelOf(item(1, "injected"), "perf")).toBe("⊕ bise");
    expect(labelOf(item(0, "tools"), "perf")).toBe("§ tools");
  });

  it("gives each role its colour kind", () => {
    expect(kindOf(item(1, "result", { ok: false }))).toBe("failed");
    expect(kindOf(item(1, "result"))).toBe("result");
    expect(kindOf(item(0, "tools"))).toBe("system");
    expect(kindOf(item(1, "unknown"))).toBe("event");
  });

  it("folds past 6 lines with the TUI's words", () => {
    const text = Array.from(
      { length: 34 },
      (_, i) => `line ${String(i)} ${"x".repeat(55)}`,
    ).join("\n");
    const long = item(5, "result", { body: { kind: "plain", text } });
    expect(folds(long)).toBe(true);
    expect(foldLabel(long)).toBe(`▸ 34 lines · ${shortBytes(text.length)}`);
    expect(
      folds(
        item(6, "you", { body: { kind: "md", text: "a\nb\nc\nd\ne\nf\n" } }),
      ),
    ).toBe(false);
    expect(folds(item(7, "you"))).toBe(false);
  });

  it("sizes bodies in UTF-8 bytes as short_bytes does", () => {
    expect(shortBytes(512)).toBe("512 B");
    expect(shortBytes(2150)).toBe("2.1 KB");
    expect(shortBytes(3 * 1024 * 1024)).toBe("3.0 MB");
    expect(sizeOf(item(1, "you", { body: { kind: "md", text: "é" } }))).toBe(
      "2 B",
    );
    expect(sizeOf(item(1, "event"))).toBe("");
  });

  it("draws compactions green, turns faint", () => {
    expect(ruleKind("compacted · 42k → 9k")).toBe("compact");
    expect(ruleKind("turn 4 · 801k in")).toBe("turn");
  });

  it("reads times as HH:MM:SS", () => {
    expect(timeOf(Date.UTC(2026, 0, 2, 14, 2, 13), "UTC")).toBe("14:02:13");
    expect(timeOf(undefined)).toBe("");
  });

  it("pretty-prints JSON bodies, leaves cut ones as written", () => {
    expect(
      codeText(
        item(1, "call", {
          body: { kind: "code", lang: "json", text: '{"a":1}' },
        }),
      ),
    ).toEqual({ text: '{\n  "a": 1\n}', lang: "json" });
    expect(
      codeText(
        item(1, "call", {
          body: { kind: "code", lang: "json", text: '{"a":', cut: true },
        }),
      ),
    ).toEqual({ text: '{"a":', lang: "json" });
    expect(
      codeText(
        item(1, "call", { body: { kind: "code", lang: "bash", text: "ls" } }),
      ),
    ).toEqual({ text: "ls", lang: "bash" });
  });

  it("draws his rows as the text sent, the file's link and the image marker kept (file-mention-wire)", () => {
    const text = 'update [The Bot.md](/u/The Bot.md) pour <image name="[Image #1]" path="/u/a.png" mime="image/png" b64="">';
    const you = item(1, "you", { body: { kind: "md", text } });
    expect(prose(you)).toBe(false);
    expect(codeText(you)).toEqual({ text, lang: "text" });
    expect(codeText(item(2, "you", { body: { kind: "md", text: "[1, 2]" } }))).toEqual({ text: "[1, 2]", lang: "text" });
    expect(prose(item(3, "assistant", { body: { kind: "md", text } }))).toBe(true);
  });

  it("puts an older page before the rows", () => {
    const rows = [item(10, "you"), item(11, "assistant")];
    expect(
      withOlder(rows, [item(8, "you"), item(9, "assistant")]).map((r) => r.seq),
    ).toEqual([8, 9, 10, 11]);
    expect(withOlder([], [item(1, "you")]).map((r) => r.seq)).toEqual([1]);
  });

  it("keeps an older page's rows below the first one's position only", () => {
    const rows = [item(10, "you"), item(11, "assistant")];
    expect(
      withOlder(rows, [item(9, "you"), item(10, "you")]).map((r) => r.pos),
    ).toEqual([9, 10, 11]);
  });

  it("steps through the hub's hits, wrapping, and counts", () => {
    expect(stepHit(0, 1, 3)).toBe(1);
    expect(stepHit(2, 1, 3)).toBe(0);
    expect(stepHit(0, -1, 3)).toBe(2);
    expect(stepHit(0, 1, 0)).toBe(-1);
    expect(findCount(0, 2)).toBe("1 of 2");
    expect(findCount(0, 0)).toBe("no match");
  });

  it("filters by kind, rules always shown", () => {
    const rows = [
      item(1, "you"),
      item(2, "thinking"),
      item(3, "event", { head: "", rule: "turn 1" }),
    ];
    expect(kindsIn(rows)).toEqual(["you", "thinking"]);
    const hidden = new Set(["thinking"] as const);
    expect(rows.filter((r) => shown(r, hidden)).map((r) => r.seq)).toEqual([
      1, 3,
    ]);
  });
});

describe("debug completion rows (designer m_37596)", () => {
  const block = (label: string | undefined, text: string) => ({
    ...(label === undefined ? {} : { label }),
    body: { kind: "plain" as const, text },
  });
  const row = (n: number, role: string, blocks: ReturnType<typeof block>[]) => ({
    n,
    role,
    head: "h",
    blocks,
    bytes: 10,
    tokens: 3,
  });
  it("a message of only calls or results goes by its first block's word", () => {
    expect(compRole(row(5, "assistant", [block("tool_use · run_typescript", "const r = 1")]))).toBe(
      "tool_use · run_typescript",
    );
    expect(compRole(row(4, "user", [block("result", "p50 2.2 s")]))).toBe("result");
    expect(compRole(row(3, "assistant", [block(undefined, "i'll bench it"), block("tool_use · bash", "ls")]))).toBe(
      "assistant",
    );
    expect(compRole(row(0, "request", [block("params", "{}")]))).toBe("request");
  });
});
