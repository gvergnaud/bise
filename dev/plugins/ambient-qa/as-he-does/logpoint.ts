// Where the driver sets a CDP logpoint on a preload function (window.bise.openExternal,
// quickLook, showInFinder): found by the function's own source, never a fixed line or script URL.
// Pure: the driver feeds it the Debugger.scriptParsed events and their sources.
//
// 789a36c1e turned `openExternal: async (url) =>` into `async (url, opts) =>` and the old
// one-parameter regex found nothing, so the PR row's logpoint was never set (11.12's lane B).
// Any parameter list matches now: the logpoint logs the first parameter.

// a Debugger.scriptParsed event, with the source Debugger.getScriptSource gave for it
export type ParsedScript = { scriptId: string; url: string; startLine?: number; startColumn?: number; source: string };

// a Debugger.setBreakpoint location, plus the parameter the condition logs
export type Logpoint = { scriptId: string; url: string; lineNumber: number; columnNumber: number; param: string };

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

// `fn: async (a, b) =>`, `fn: async a =>`, `fn: (a) =>`, `fn: async function (a) {`,
// `async fn(a, b) {`: the body's first character and the first parameter's name
export function findFn(src: string, fn: string): { at: number; param: string } | null {
  const name = escape(fn);
  const forms = [
    new RegExp(`\\b${name}\\s*:\\s*(?:async\\s*)?(?:\\(\\s*([A-Za-z_$][\\w$]*)[^)]*\\)|([A-Za-z_$][\\w$]*))\\s*=>\\s*`),
    new RegExp(`\\b${name}\\s*:\\s*(?:async\\s+)?function\\b[^(]*\\(\\s*([A-Za-z_$][\\w$]*)[^)]*\\)\\s*\\{\\s*`),
    new RegExp(`(?:^|[\\s,{])(?:async\\s+)?${name}\\s*\\(\\s*([A-Za-z_$][\\w$]*)[^)]*\\)\\s*\\{\\s*`),
  ];
  for (const re of forms) {
    const m = re.exec(src);
    const param = m?.[1] ?? m?.[2];
    if (m && param) return { at: m.index + m[0].length, param };
  }
  return null;
}

// the first script that defines fn, and the CDP location of its body (a script's lines count
// from its startLine; its first line's columns from its startColumn)
export function pickLogpoint(scripts: ParsedScript[], fn: string): Logpoint | null {
  for (const s of scripts) {
    const hit = findFn(s.source, fn);
    if (!hit) continue;
    const head = s.source.slice(0, hit.at);
    const line = head.split('\n').length - 1;
    const col = hit.at - (head.lastIndexOf('\n') + 1);
    return {
      scriptId: s.scriptId, url: s.url, param: hit.param,
      lineNumber: line + (s.startLine ?? 0),
      columnNumber: col + (line === 0 ? (s.startColumn ?? 0) : 0),
    };
  }
  return null;
}

// why no logpoint: the scripts looked at, so a red row says what the driver saw
export function noLogpoint(scripts: ParsedScript[], fn: string): string {
  const urls = scripts.map((s) => s.url || `(${s.scriptId})`);
  return `no script defines ${fn} among ${scripts.length}: ${urls.slice(0, 12).join(', ')}`.slice(0, 400);
}
