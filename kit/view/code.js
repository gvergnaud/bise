// generated from code.ts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// bise kit · the one code table (code-view, architect m_66499): which language a file is (its
// name's ending, a few whole names, else its shebang), what that language is called on screen,
// and the look of each of prism's token types as one of bise's syntax roles. The desktop's code
// blocks and diffs (apps/desktop thread/Code.tsx, review/syntax.ts) and the Code app's view
// (plugins/code) read it, so a file in its tab looks like the same code in a message. Its
// languages are prism's ids (prism-react-renderer's in the window, prismjs' in the view: the same
// grammars). The Code app's view.json claims exactly these endings and names (a test). Pure.
/** A file's ending (lowercase, no dot) → its prism language. */
export const LANGS = {
    ts: "typescript", mts: "typescript", cts: "typescript", tsx: "tsx",
    js: "javascript", mjs: "javascript", cjs: "javascript", jsx: "jsx",
    py: "python", pyi: "python", bend: "python",
    rs: "rust", go: "go", swift: "swift", kt: "kotlin", kts: "kotlin", java: "java",
    c: "c", h: "c", cc: "cpp", cpp: "cpp", cxx: "cpp", hh: "cpp", hpp: "cpp", hxx: "cpp",
    m: "objectivec", mm: "objectivec", cs: "csharp", rb: "ruby", php: "php",
    sh: "bash", bash: "bash", zsh: "bash",
    sql: "sql", json: "json", jsonl: "json", yaml: "yaml", yml: "yaml", toml: "toml", ini: "ini",
    html: "markup", htm: "markup", xml: "markup", svg: "markup", plist: "markup",
    css: "css", scss: "scss", md: "markdown", markdown: "markdown",
    graphql: "graphql", gql: "graphql", lua: "lua", mk: "makefile",
};
/** A whole file name (lowercase) with no ending of its own → its prism language. */
export const NAMES = {
    dockerfile: "docker", containerfile: "docker", makefile: "makefile", gnumakefile: "makefile",
};
/** What a language is called on screen (the document's kind: 'TypeScript · v10'). */
export const TITLES = {
    typescript: "TypeScript", tsx: "TypeScript", javascript: "JavaScript", jsx: "JavaScript",
    python: "Python", rust: "Rust", go: "Go", swift: "Swift", kotlin: "Kotlin", java: "Java",
    c: "C", cpp: "C++", objectivec: "Objective-C", csharp: "C#", ruby: "Ruby", php: "PHP",
    bash: "Shell script", sql: "SQL", json: "JSON", yaml: "YAML", toml: "TOML", ini: "INI",
    markup: "HTML", css: "CSS", scss: "SCSS", markdown: "Markdown", graphql: "GraphQL", lua: "Lua",
    makefile: "Makefile", docker: "Dockerfile",
};
/** A file with no language of its own. */
export const PLAIN = "Plain text";
/** A shebang's program → its prism language (`#!/usr/bin/env python3`). */
const SHEBANGS = [
    [/^(node|nodejs|deno|bun)$/, "javascript"],
    [/^(ts-node|tsx)$/, "typescript"],
    [/^python[\d.]*$/, "python"],
    [/^(sh|bash|zsh|dash|ksh)$/, "bash"],
    [/^ruby$/, "ruby"],
    [/^php$/, "php"],
];
/** The program a first line's shebang runs (`#!/usr/bin/env -S python3 -u` → python3), or ''. Pure. */
function shebangOf(first) {
    if (!first.startsWith("#!"))
        return "";
    const words = first.slice(2).trim().split(/\s+/);
    const prog = (words[0] ?? "").split("/").pop() ?? "";
    if (prog !== "env")
        return prog;
    return words.slice(1).find((w) => !w.startsWith("-") && !w.includes("=")) ?? "";
}
/**
 * A file's prism language: its ending, else its whole name, else its first line's shebang;
 * undefined: plain text. `bend.ts` and `Dockerfile` and `#!/usr/bin/env python3` all say. Pure.
 */
export function langOf(name, first = "") {
    const base = (name.slice(name.lastIndexOf("/") + 1)).toLowerCase();
    const dot = base.lastIndexOf(".");
    const byEnd = dot > 0 ? LANGS[base.slice(dot + 1)] : undefined;
    if (byEnd !== undefined)
        return byEnd;
    const byName = NAMES[base] ?? (base.startsWith("dockerfile.") ? "docker" : undefined);
    if (byName !== undefined)
        return byName;
    const prog = shebangOf(first);
    return prog === "" ? undefined : SHEBANGS.find(([re]) => re.test(prog))?.[1];
}
/** What a language is called on screen; none (or one we don't name): 'Plain text'. Pure. */
export function titleOf(lang) {
    return (lang === undefined ? undefined : TITLES[lang]) ?? PLAIN;
}
/** An ending named apart from its grammar's language (a .bend file is colored as Python). */
const END_TITLES = { bend: "Bend", xml: "XML", svg: "XML", plist: "XML" };
/** What a file is called on screen, its document's kind ('TypeScript', 'Bend', 'Plain text'). Pure. */
export function fileTitle(name, first = "") {
    const base = name.slice(name.lastIndexOf("/") + 1).toLowerCase();
    const dot = base.lastIndexOf(".");
    return (dot > 0 ? END_TITLES[base.slice(dot + 1)] : undefined) ?? titleOf(langOf(name, first));
}
/**
 * prism's token types on bise's roles, in order (a later type of a token wins over an earlier
 * one, prism-react-renderer's rule): keywords bold, comments italic, punctuation dim.
 */
export const ROLES = [
    { types: ["comment", "prolog", "doctype", "cdata"], hue: "comment", italic: true },
    { types: ["keyword", "builtin", "important", "atrule"], hue: "keyword", bold: true },
    { types: ["string", "char", "attr-value", "regex", "url"], hue: "string" },
    { types: ["number", "boolean", "constant", "symbol"], hue: "number" },
    { types: ["function", "function-variable", "method"], hue: "call" },
    { types: ["class-name", "tag", "selector", "namespace", "type"], hue: "type" },
    { types: ["punctuation", "operator"], hue: "text-2" },
    { types: ["inserted"], hue: "ok" },
    { types: ["deleted"], hue: "err", strike: true },
    // ```md: a heading as a keyword (prism's alias), **bold** bold, *italic* italic, ~~strike~~
    // struck, `code` and links as strings, the marks dim as punctuation
    { types: ["bold"], bold: true },
    { types: ["italic"], italic: true },
    { types: ["strike"], strike: true },
    { types: ["code-snippet", "code-block"], hue: "string", normal: true },
];
/** prism aliases md's `code` as a keyword: its own type wins (the TUI's code is green). */
export const WINS = ["code-snippet", "code-block"];
/** The TUI's built-in type words (rust/tui/src/syntax.rs RUST_TYPES, PY_TYPES): prism calls them
 * keywords or builtins, bise draws them as types (gold, not bold). */
export const TYPE_WORDS = new Set([
    "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64",
    "int", "float", "list", "dict", "set", "tuple", "bytes", "object",
]);
/** A token prism calls a keyword or a builtin that bise draws as a type. Pure. */
export function typeWord(types, content) {
    return (types.includes("keyword") || types.includes("builtin")) && TYPE_WORDS.has(content.trim());
}
/** A token's merged role (its types' roles in order, inner over outer, WINS last, a type word as a
 * type): what the window draws as a style and the view as classes. Pure. */
export function roleOf(types, content) {
    const byType = (t) => ROLES.filter((r) => r.types.includes(t));
    const all = [...types, ...types.filter((t) => WINS.includes(t))].flatMap(byType);
    const merged = all.reduce((acc, r) => ({ ...acc, ...r, types: [] }), { types: [] });
    return typeWord(types, content) ? { ...merged, hue: "type", bold: false, normal: true } : merged;
}
