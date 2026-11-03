// What people read: the `summary` line the TUI shows after ↖ (designer,
// m_3616), and the pages the extension refuses (design §5.1). Pure.

import { uploadSummary, baseName } from "./upload.js";

const filesList = (f: unknown): string[] => (typeof f === "string" ? [f] : Array.isArray(f) ? f.map(String) : []);

/** "https://www.amazon.fr/x" → "amazon.fr"; no host → "". */
/** An element as a summary names it (a snapshot's node). */
export type Named = { role: string; name?: string };
/** The act args a summary reads. */
export type SummaryArgs = { value?: unknown; text?: unknown; direction?: string; amount?: number; files?: unknown };
/** A refused URL: the C1 message's start, the summary's reason, the place. */
export type Refused = { why: string; short: string; place: string };

export function hostOf(url: string | URL | null | undefined) {
  try {
    // (none: new URL throws, the catch says "")
    const u = new URL(url as string | URL);
    if (u.protocol === "file:") return "file";
    return u.host.replace(/^www\./, "");
  } catch {
    return "";
  }
}

/** A label in a summary: one line, at most 32 cells, ends in … when cut. */
export function label(s: unknown) {
  const t = String(s ?? "").replace(/\s+/g, " ").trim();
  return [...t].length > 32 ? [...t].slice(0, 31).join("") + "…" : t;
}

const q = (s: unknown) => `"${label(s)}"`;
const at = (host: string | null | undefined) => (host ? ` · ${host}` : "");

// No name: "typed in a search box".
const NOUN: Record<string, string> = { searchbox: "a search box", textbox: "a text box", combobox: "a combo box", spinbutton: "a number box" };

/** What the element is called in a summary. */
function what(el: Named | null | undefined) {
  if (!el) return "";
  if (el.name) return q(el.name);
  return NOUN[el.role] || `a ${el.role}`;
}

/**
 * The user-facing line of a done action.
 * @param action C1 action; el {role, name} or null; args the act args;
 *        host of the page (after the action for goto).
 */
export function summary(action: string, el: Named | null | undefined, args: SummaryArgs, host: string | null | undefined, keyText = "") {
  switch (action) {
    case "click": return `clicked ${what(el) || "the page"}${at(host)}`;
    case "fill":
    case "type": return `typed in ${what(el) || "the page"}${at(host)}`;
    case "press": return `pressed ${keyText}${at(host)}`;
    case "select": return `picked ${q(args.value ?? args.text ?? "")} in ${what(el)}${at(host)}`;
    case "check": return `${args.value === false ? "unchecked" : "checked"} ${what(el)}${at(host)}`;
    case "hover": return `pointed at ${what(el)}${at(host)}`;
    case "scroll": return el && !args.direction ? `scrolled to ${what(el)}${at(host)}` : `scrolled ${args.direction || "down"}${at(host)}`;
    case "goto": return `went to ${host || "the page"}`;
    case "close": return `closed the tab${at(host)}`;
    case "wait": return `waited for ${el ? what(el) : args.text !== undefined ? q(args.text) : `${Math.round((args.amount ?? 0) / 100) / 10} s`}${at(host)}`;
    case "read": return `read ${el ? what(el) : "the page"}${at(host)}`;
    case "open": return `opened a tab${at(host)}`;
    case "set_input_files": return uploadSummary(filesList(args.files).map(baseName), what(el), host);
    case "expect_chooser": return `waiting for a file chooser${at(host)}`;
    case "wait_chooser": return `a file chooser opened${at(host)}`;
    default: return `${action}${at(host)}`;
  }
}

const VERB: Record<string, string> = { set_input_files: "attach files to", expect_chooser: "wait for a file chooser", wait_chooser: "wait for a file chooser", click: "click", fill: "type in", type: "type in", press: "press", select: "pick in", check: "check", hover: "point at", scroll: "scroll", goto: "go to", close: "close", wait: "wait for", read: "read", open: "open" };

/**
 * The user-facing line of a failed action (no ✗: the TUI adds it):
 * `couldn't click "Add to cart": something covers it`, `couldn't find "Email"`.
 */
export function failure(action: string, target: string | null | undefined, code: string, reason = "") {
  const t = target ? ` ${target}` : "";
  if (code === "not_found" || code === "stale_ref") return `couldn't find${t || " it"}`;
  const why = reason || ({
    ambiguous: "several match",
    paused: "you're using this tab",
    stopped: "you stopped it",
    refused: "bise doesn't drive this page",
    timeout: "it took too long",
    bad_args: "the request was wrong",
  } as Record<string, string>)[code] || code;
  return `couldn't ${VERB[action] || action}${t}: ${why}`;
}

/**
 * A failure line ends with its place like a success line (designer m_3904
 * #6): `couldn't click "Add to cart": something covers it · amazon.fr`.
 * Not twice when the line already names the host (open, goto).
 */
export function withPlace(line: string, host: string | null | undefined) {
  return host && !line.includes(host) ? `${line}${at(host)}` : line;
}

// Pages the extension never drives (design §5.1; the broker checks too).
const REFUSED_SCHEMES = ["chrome:", "chrome-extension:", "chrome-untrusted:", "chrome-search:", "devtools:", "edge:", "brave:", "vivaldi:", "opera:", "arc:", "view-source:", "extension:"];
const REFUSED_HOSTS = ["chromewebstore.google.com", "microsoftedge.microsoft.com", "addons.opera.com", "chrome.google.com"];

/** A reason when the URL is refused, else null. */
export function refusal(url: string | null | undefined) {
  return refused(url)?.why ?? null;
}

/**
 * A refused URL: { why (the C1 message's start), short (the summary's
 * reason), place (`chrome://settings`, `chromewebstore.google.com`) },
 * else null.
 */
export function refused(url: string | null | undefined): Refused | null {
  let u: URL;
  try { u = new URL(url as string); } catch { return null; }
  // `chrome://settings/x` → "chrome://settings"
  const page = () => `${u.protocol}//${u.host || u.pathname.split("/")[0]}`;
  if (REFUSED_SCHEMES.includes(u.protocol)) {
    return { why: `the browser doesn't let extensions drive ${u.protocol}// pages`, short: "the browser doesn't allow it", place: page() };
  }
  if (u.protocol === "about:" && u.href !== "about:blank") return { why: "the browser doesn't let extensions drive about: pages", short: "the browser doesn't allow it", place: u.href };
  const host = u.hostname;
  if (host === "chrome.google.com" && !u.pathname.startsWith("/webstore")) return null;
  if (REFUSED_HOSTS.includes(host) && (host !== "microsoftedge.microsoft.com" || u.pathname.startsWith("/addons"))) {
    return { why: "the browser doesn't let extensions drive its extension store", short: "the browser doesn't allow it", place: host };
  }
  if (host === "accounts.google.com" && /\/challenge\/(pwd|password)/.test(u.pathname)) return { why: "the user types Google passwords themselves", short: "the user signs in", place: host };
  return null;
}
