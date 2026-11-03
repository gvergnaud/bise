// Driving a tab: finding an element, the overlay (cursor, his touch), the
// actions, the screenshot. Uploads are upload.ts's.

import { ACTIONS, LOCATOR_KEYS, CuError, fail, sleep, agents, tabs, note, agentOf, tabsOf, forget } from "./state.js";
import { toError } from "./host.js";
import { refuseUrl, normalUrl, waitLoaded, owned, serial } from "./tabs.js";
import { cdp, attach, detach, lostTab, evaluate, callOn, takeSnapshot, onTab, PDF_NOTE, isPdf } from "./cdp.js";
import { pick, diff, describeLocator, TEXT_ROLES } from "../lib/ax.js";
import { parseKeys, chordEvents, keyLabel } from "../lib/keys.js";
import { summary, failure, withPlace, hostOf, label } from "../lib/text.js";
import { jpegSize } from "../lib/jpeg.js";
import { setInputFiles, expectChooser, waitChooser } from "./upload.js";
import type { Entry, Locator, Picked } from "../lib/ax.js";
import type { Chord } from "../lib/keys.js";
import type { Tab } from "./state.js";
import type { Args } from "./tabs.js";
import type { Evaluated, RemoteObject } from "./cdp.js";

/** An act's args (C1): the action, its target, its own fields. */
export type ActArgs = Args & {
  action?: string; ref?: string; locator?: Locator; text?: string; value?: unknown; keys?: string; direction?: string;
  amount?: number; clear?: boolean; full_page?: boolean; max_width?: number; files?: unknown; chooser?: unknown; [k: string]: unknown;
};
/** Where a click lands, or why not. */
type Point = { ok: true; x: number; y: number; box: Box } | { ok: false; reason: string; covered?: boolean };
type Box = { x: number; y: number; width: number; height: number };
/** Page.getLayoutMetrics's visual viewport, the fields read here. */
type Viewport = { cssVisualViewport: { clientWidth: number; clientHeight: number; pageX: number; pageY: number; scale?: number } };
/** Why resolve() has not found its element yet. */
type Problem = { code: string; reason?: string; candidates?: (string | undefined)[]; count?: number };

// ---------------------------------------------------------------- finding an element

const quadArea = (q: number[]) => Math.abs((q[2] - q[0]) * (q[5] - q[1]) - (q[4] - q[0]) * (q[3] - q[1]));

/** Where to click the element: { ok, x, y, box } or { ok: false, reason }. */
async function pointOf(tabId: number, backendNodeId: number | undefined): Promise<Point> {
  try {
    await cdp(tabId, "DOM.scrollIntoViewIfNeeded", { backendNodeId });
  } catch {
    // text nodes and some svg: getContentQuads still works
  }
  let quads: number[][];
  try {
    ({ quads } = await cdp<{ quads: number[][] }>(tabId, "DOM.getContentQuads", { backendNodeId }));
  } catch (e) {
    // a node the page replaced since the snapshot (a region re-rendered
    // every 30 ms) is not "invisible" (launch #2): say what happened
    const gone = /No node|not found|does not belong|Could not find/i.test(String((e as Error)?.message || e)) ||
      (await callOn(tabId, backendNodeId!, function (this: Node) { return !this.isConnected; }).catch(() => true));
    if (gone) return { ok: false, reason: "the page keeps replacing it (it re-renders faster than bise can act); try a stable element near it" };
    return { ok: false, reason: "it isn't visible" };
  }
  const q = quads.find((x) => quadArea(x) > 1);
  if (!q) return { ok: false, reason: "it isn't visible" };
  const xs = [q[0], q[2], q[4], q[6]], ys = [q[1], q[3], q[5], q[7]];
  const box = { x: Math.min(...xs), y: Math.min(...ys), width: Math.max(...xs) - Math.min(...xs), height: Math.max(...ys) - Math.min(...ys) };
  const { cssVisualViewport: vv } = await cdp<Viewport>(tabId, "Page.getLayoutMetrics");
  const left = Math.max(box.x, 0), top = Math.max(box.y, 0);
  const right = Math.min(box.x + box.width, vv.clientWidth), bottom = Math.min(box.y + box.height, vv.clientHeight);
  if (right - left < 1 || bottom - top < 1) return { ok: false, reason: "it is off screen" };
  const x = Math.round((left + right) / 2), y = Math.round((top + bottom) / 2);
  // Does a click there land on it? (a cookie banner, a modal…)
  try {
    await cdp(tabId, "DOM.getDocument", { depth: 0 });
    const hit = await cdp<{ backendNodeId: number }>(tabId, "DOM.getNodeForLocation", { x, y, includeUserAgentShadowDOM: true, ignorePointerEventsNone: false });
    if (hit.backendNodeId !== backendNodeId) {
      const { object } = await cdp<{ object: RemoteObject }>(tabId, "DOM.resolveNode", { backendNodeId: hit.backendNodeId, objectGroup: "bise" });
      const inside = await callOn(tabId, backendNodeId!, function (this: Node, o: Node) {
        const t = this.nodeType === 1 ? this : this.parentElement;
        const up = (a: Node | null, b: Node | null) => { for (let n = a; n; n = n.parentNode || (n as ShadowRoot).host) if (n === b) return true; return false; };
        return !!t && (up(o, t) || up(t, o));
      }, [{ objectId: object.objectId }]);
      // In a background tab, DOM.getNodeForLocation can answer from before
      // the scroll just above (no frame was drawn since): a button below
      // the fold read as covered (cu-upload, a 900 px page in a 613 px
      // tab). The page's own hit test (it lays out first) decides.
      const seen = inside === false && (await callOn(tabId, backendNodeId!, function (this: Node, px: number, py: number) {
        const t = this.nodeType === 1 ? this : this.parentElement;
        const o = document.elementFromPoint(px, py);
        const up = (a: Node | null, b: Node | null) => { for (let n = a; n; n = n.parentNode || (n as ShadowRoot).host) if (n === b) return true; return false; };
        return !!t && !!o && (up(o, t) || up(t, o));
      }, [{ value: x }, { value: y }]).catch(() => false));
      if (inside === false && !seen) return { ok: false, reason: "something covers it", covered: true };
    }
  } catch (e) {
    // hit test unavailable: click anyway
    note({ hitTest: String((e as Error).message || e) });
  }
  return { ok: true, x, y, box };
}

function targetLabel(args: ActArgs, el: Entry | null | undefined) {
  if (el) return el.name ? `"${label(el.name)}"` : "";
  if (args.locator) {
    const l = args.locator;
    const w = l.name ?? l.text ?? l.label ?? l.name_re ?? l.text_re;
    return w !== undefined ? `"${label(w)}"` : l.role || "";
  }
  return args.ref ? `${args.ref}` : "";
}

/**
 * Wait (≤ deadline) until the ref or locator is one element, visible
 * and enabled when asked. Returns { entry, snap, point? }.
 */
async function resolve(tabId: number, t: Tab, args: ActArgs, deadline: number, { visible = true, enabled = false } = {}): Promise<{ entry: Entry; snap: Awaited<ReturnType<typeof takeSnapshot>>; point?: Point & { ok: true } }> {
  const loc = args.locator;
  if (args.ref && loc) fail("bad_args", "give a ref or a locator, not both");
  if (loc && typeof loc !== "object") fail("bad_args", "locator must be an object like {role, name}");
  // an unknown key was ignored: {css: ".stage"} matched every element and
  // came back ambiguous (launch #5)
  const unknown = loc ? Object.keys(loc).filter((k) => !LOCATOR_KEYS.has(k)) : [];
  if (unknown.length) fail("bad_args", `unknown locator key${unknown.length > 1 ? "s" : ""} ${unknown.join(", ")}: use ${[...LOCATOR_KEYS].join(", ")} (no CSS selectors: find the element in a snapshot)`, { reason: `${unknown[0]} isn't a locator key` });
  const desc = args.ref ? args.ref : describeLocator(loc);
  for (;;) {
    const snap = await takeSnapshot(tabId, t);
    // (problem: set on each path that has no point; fail() throws)
    let entry: Entry | null = null, problem!: Problem | Picked;
    if (args.ref) {
      const n = Number(/^e?(\d+)$/.exec(String(args.ref))?.[1]);
      if (!n) fail("bad_args", `bad ref ${JSON.stringify(args.ref)}; refs look like "e12"`);
      const backendId = t.byRef.get(n);
      if (!backendId) {
        if (n < t.nextRef) fail("stale_ref", `${args.ref} is from before the page changed; take a new snapshot`);
        fail("bad_args", `there is no ${args.ref} on this page; take a snapshot first`);
      }
      entry = snap.entries.find((e) => e.backendId === backendId) || null;
      if (!entry) {
        try {
          await cdp(tabId, "DOM.resolveNode", { backendNodeId: backendId });
        } catch {
          fail("stale_ref", `${args.ref} is gone from the page; take a new snapshot`);
        }
        problem = { code: "timeout", reason: "it isn't visible" };
      }
    } else {
      const p = pick(snap.entries, loc || {});
      if (p.entry) entry = p.entry;
      else problem = p;
    }
    if (entry) {
      if (enabled && entry.disabled) problem = { code: "timeout", reason: "it's disabled" };
      else if (!visible) return { entry, snap };
      else {
        const pt = await pointOf(tabId, entry.backendId);
        if (pt.ok) return { entry, snap, point: pt };
        problem = { code: "timeout", reason: pt.reason };
      }
    }
    if (Date.now() >= deadline) {
      const ms = args.timeout_ms ?? 5000;
      if (problem.code === "not_found") fail("not_found", `nothing matches ${desc} (waited ${ms} ms); the closest elements are in candidates`, { candidates: problem.candidates });
      if (problem.code === "ambiguous") fail("ambiguous", `${problem.count} elements match ${desc}; add nth or a more exact name`, { candidates: problem.candidates, reason: "several match" });
      const extra = (problem as Problem).reason === "something covers it" ? " (a popup or banner?); close it first" : "";
      fail("timeout", `${desc}: ${(problem as Problem).reason} after ${ms} ms${extra}`, { reason: (problem as Problem).reason, candidates: entry ? [entry.line] : undefined });
    }
    await sleep(120);
  }
}

// ---------------------------------------------------------------- the overlay (cursor, his touch)

async function ensureOverlay(tabId: number) {
  const t = tabs.get(tabId);
  if (!t || t.overlayNavs === t.navs) return;
  try {
    await chrome.scripting.executeScript({ target: { tabId }, files: ["overlay.js"] });
    t.overlayNavs = t.navs;
  } catch {
    // a page scripts can't run on
  }
}

async function cursor(tabId: number, agent: string, point: { x: number; y: number }, ring: boolean) {
  try {
    await ensureOverlay(tabId);
    const tab = await chrome.tabs.get(tabId);
    const shown = chrome.tabs.sendMessage(tabId, { bise: "cursor", x: point.x, y: point.y, agent, ring });
    // Let the user see the glide before the click, only when the tab is in view.
    if (tab.active) await Promise.race([shown, sleep(400)]);
    else shown.catch(() => {});
  } catch {
    // purely visual
  }
}

chrome.runtime.onMessage.addListener((msg: { bise?: string; kind?: string } | undefined, sender) => {
  if (msg?.bise !== "input" || !sender.tab) return;
  const t = tabs.get(sender.tab.id!);
  // CDP input is trusted too: ignore what arrives while (or right after) we act.
  // Only a tab in view can get the user's input (a hidden agent tab can't).
  if (!t || !sender.tab.active || t.acting || Date.now() < t.actingUntil) return;
  touched(sender.tab.id!, t, msg.kind as string);
});

/**
 * The user looked at or used an agent's tab (activated it, clicked or typed
 * in it). It stays the agent's and the agent keeps driving it: taking over
 * is his explicit act (bise's window, ctl `pause`). It only changes what a
 * drop does: the tab is left to him, not closed under his eyes.
 */
function touched(tabId: number, t: Tab, why: string) {
  note({ touched: tabId, why });
  t.userTouched = true;
}

// ---------------------------------------------------------------- act

async function mouse(tabId: number, x: number, y: number, kind: string) {
  const t0 = Date.now();
  await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
  if (kind === "click") {
    await cdp(tabId, "Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", buttons: 1, clickCount: 1 });
    await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", buttons: 0, clickCount: 1 });
  }
  // The hidden-tab trap (design §3) shows here as ~5000 ms.
  note({ input: kind, ms: Date.now() - t0 });
}

async function keys(tabId: number, chords: Chord[]) {
  for (const c of chords) for (const ev of chordEvents(c)) await cdp(tabId, "Input.dispatchKeyEvent", ev);
}

function focusFn(selectAll: boolean) {
  return `function () {
    const el = this.nodeType === 1 ? this : this.parentElement;
    el.focus();
    if (${selectAll}) {
      if (typeof el.select === "function") { try { el.select(); return; } catch (e) {} }
      if (el.isContentEditable) { const r = document.createRange(); r.selectNodeContents(el); const s = getSelection(); s.removeAllRanges(); s.addRange(r); }
    } else if (typeof el.setSelectionRange === "function") {
      try { const n = el.value.length; el.setSelectionRange(n, n); } catch (e) {}
    }
  }`;
}

// the functions below run in the page (callOn sends their source): `this` is the element
type El = HTMLElement & Partial<HTMLInputElement>;

function isPasswordFn(this: El) {
  return this.tagName === "INPUT" && this.type === "password";
}

/** Passwords are the user's (ship plan §6): never typed by an agent, on any site. */
function refusePassword() {
  fail("refused", "the user types passwords himself; ask him to sign in, then go on", { reason: "he types passwords himself" });
}

function readFn(this: El & Partial<HTMLSelectElement>) {
  if (this.nodeType === 3) return this.textContent;
  const el = this;
  if (el.tagName === "INPUT" && (el.type as string) === "password") return "•••";
  if (el.tagName === "SELECT") return el.selectedOptions![0]?.label ?? "";
  if (el.tagName === "INPUT" || el.tagName === "TEXTAREA") return el.value;
  return el.innerText ?? el.textContent ?? "";
}

function selectFn(this: HTMLSelectElement, v: unknown) {
  if (this.tagName !== "SELECT") return { error: "not_select" };
  const opts = [...this.options];
  const want = String(v);
  const o = opts.find((x) => x.value === want) || opts.find((x) => x.label.trim() === want) || opts.find((x) => x.label.trim().toLowerCase().includes(want.toLowerCase()));
  if (!o) return { error: "no_option", options: opts.slice(0, 10).map((x) => x.label.trim()) };
  this.value = o.value;
  this.dispatchEvent(new Event("input", { bubbles: true }));
  this.dispatchEvent(new Event("change", { bubbles: true }));
  return { label: o.label.trim() };
}

function scrollFn(this: Node | null, dx: number, dy: number) {
  let n = this && this.nodeType ? (this.nodeType === 1 ? (this as Element) : this.parentElement) : null;
  const scrollable = (e: Element) => {
    const s = getComputedStyle(e);
    return (e.scrollHeight > e.clientHeight + 1 && /(auto|scroll|overlay)/.test(s.overflowY)) || (e.scrollWidth > e.clientWidth + 1 && /(auto|scroll|overlay)/.test(s.overflowX));
  };
  while (n && n !== document.body && n !== document.documentElement && !scrollable(n)) n = n.parentElement;
  const box = (n && n !== document.body && n !== document.documentElement ? n : document.scrollingElement)!;
  const before = [box.scrollLeft, box.scrollTop].join();
  box.scrollBy(dx, dy);
  return before !== [box.scrollLeft, box.scrollTop].join();
}

function viewSize(this: Node | null) {
  let n = this && this.nodeType ? (this.nodeType === 1 ? (this as Element) : this.parentElement) : null;
  while (n && n !== document.body && n !== document.documentElement && !(n.scrollHeight > n.clientHeight + 1 && /(auto|scroll|overlay)/.test(getComputedStyle(n).overflowY))) n = n.parentElement;
  const box = n && n !== document.body && n !== document.documentElement ? n : null;
  const sig = () => [scrollX, scrollY, box ? box.scrollLeft : 0, box ? box.scrollTop : 0].join();
  return { w: box ? box.clientWidth : innerWidth, h: box ? box.clientHeight : innerHeight, sig: sig() };
}

/** Wait for the page to settle after an action, then snapshot it. */
type Snap = Awaited<ReturnType<typeof takeSnapshot>>;

async function settle(tabId: number, t: Tab, deadline: number) {
  await sleep(60);
  while (Date.now() < deadline + 10_000) {
    const tab = await chrome.tabs.get(tabId);
    if (tab.status !== "loading") break;
    await sleep(100);
  }
  let prev = await takeSnapshot(tabId, t);
  // a page that never stops changing (a clock, a region re-rendered every
  // 30 ms) never settles: 8 more snapshots of a big tree took a click to
  // 15 s (launch's hero-cine). At most ~1.5 s here; the diff says the rest.
  const until = Date.now() + 1500;
  for (let i = 0; i < 8 && Date.now() < until; i++) {
    await sleep(100);
    const next = await takeSnapshot(tabId, t);
    if (next.navs === prev.navs && next.lines.join("\n") === prev.lines.join("\n")) return next;
    prev = next;
  }
  return prev;
}

function changedText(before: Snap, after: Snap) {
  if (after.navs !== before.navs || after.url !== before.url && hostOf(after.url) !== hostOf(before.url)) {
    const lines = [after.head, ...after.lines.slice(0, 19).map((l) => l)];
    if (after.lines.length > 19) lines[19] = `… ${after.lines.length - 18} more lines (snapshot for all)`;
    return lines.slice(0, 20).join("\n");
  }
  return diff(before.lines, after.lines);
}

async function act(agent: string, args: ActArgs) {
  const { tabId, t } = await owned(agent, args.target);
  // (fail throws: a known action past this line)
  const action = args.action as string;
  if (!ACTIONS.has(action)) fail("bad_args", `unknown action ${JSON.stringify(action)}; one of ${[...ACTIONS].join(", ")}`);
  const place = async () => hostOf((await chrome.tabs.get(tabId).catch(() => ({}) as chrome.tabs.Tab)).url || "");
  // close on a paused tab: the user is in it, so don't close it under
  // him; give it to him instead (out of the group, no longer the agent's,
  // no longer counted in its 5 tabs). A paused tab stuck the agent at its
  // limit (launch's re-test #3).
  if (t.paused && action === "close") {
    const where = await place();
    await detach(tabId, t);
    forget(tabId);
    await chrome.tabs.ungroup([tabId]).catch(() => {});
    return { ok: true, changed: "", summary: withPlace("gave the tab to you", where), handed_over: true };
  }
  if (t.paused) fail("paused", "the user is using this tab; wait until they give it back, or ask them", { summary: withPlace(failure(action, targetLabel(args, undefined), "paused"), await place()) });
  return serial(t, async () => {
    let el: Entry | null = null;
    try {
      t.acting = true;
      await attach(tabId, t);
      const out = await doAct(tabId, t, agent, action, args, (e) => (el = e));
      return out;
    } catch (raw) {
      const e = (await lostTab(agent, tabId, raw)) || raw;
      if (e instanceof CuError && !e.summary) e.summary = failure(action, targetLabel(args, el), e.code, e.reason as string | undefined);
      if (!(e instanceof CuError)) {
        const err = toError(e);
        throw new CuError(err.code, err.message, { summary: withPlace(failure(action, targetLabel(args, el), err.code), await place()) });
      }
      if (e.summary) e.summary = withPlace(e.summary as string, await place());
      throw e;
    } finally {
      t.acting = false;
      t.actingUntil = Date.now() + 600;
    }
  });
}

// (a visible target always has its point: need() with visible true; before: every
// path that reaches the diff took a snapshot first)
async function doAct(tabId: number, t: Tab, agent: string, action: string, args: ActArgs, seen: (e: Entry) => void) {
  const started = Date.now();
  const timeout = Math.min(Math.max(Number.isFinite(args.timeout_ms) ? args.timeout_ms! : 5000, 0), 120_000);
  const deadline = Date.now() + timeout;
  const hasTarget = !!(args.ref || args.locator);
  const need = (opts: { visible?: boolean; enabled?: boolean }) => resolve(tabId, t, args, deadline, opts).then((r) => (seen(r.entry), r as typeof r & { point: Point & { ok: true } }));
  let el: Entry | null = null, before: Snap | null = null, extra = "";
  // fields of this action's result besides the usual ones (an upload's `via`)
  let more: Record<string, unknown> = {};

  switch (action) {
    case "click": {
      if (!hasTarget) fail("bad_args", "click needs a ref or a locator");
      const r = await need({ visible: true, enabled: true });
      ({ entry: el, snap: before } = r);
      await cursor(tabId, agent, r.point, true);
      await mouse(tabId, r.point.x, r.point.y, "click");
      break;
    }
    case "hover": {
      if (!hasTarget) fail("bad_args", "hover needs a ref or a locator");
      const r = await need({ visible: true });
      ({ entry: el, snap: before } = r);
      await cursor(tabId, agent, r.point, false);
      await mouse(tabId, r.point.x, r.point.y, "move");
      break;
    }
    case "fill":
    case "type": {
      const text = args.text ?? args.value;
      if (typeof text !== "string") fail("bad_args", `${action} needs text`);
      if (hasTarget) {
        const r = await need({ visible: true, enabled: true });
        ({ entry: el, snap: before } = r);
        if (!TEXT_ROLES.has(el.role) && !el.editable) fail("bad_args", `${el.line} is not a text field`, { reason: "it isn't a text field" });
        if (await callOn(tabId, el.backendId, isPasswordFn)) refusePassword();
        await cursor(tabId, agent, r.point, true);
        await callOn(tabId, el.backendId, focusFn(action === "fill"));
      } else if (action === "fill") {
        fail("bad_args", "fill needs a ref or a locator");
      } else {
        before = await takeSnapshot(tabId, t);
        const focused = await cdp<Evaluated>(tabId, "Runtime.evaluate", { expression: "document.activeElement?.tagName === 'INPUT' && document.activeElement.type === 'password'", returnByValue: true });
        if (focused.result?.value === true) refusePassword();
      }
      if (text === "" && action === "fill") await keys(tabId, parseKeys("Delete"));
      else if (text !== "") await cdp(tabId, "Input.insertText", { text });
      break;
    }
    case "press": {
      const chords = parseKeys(args.keys ?? args.text);
      if (hasTarget) {
        const r = await need({ visible: true });
        ({ entry: el, snap: before } = r);
        await callOn(tabId, el.backendId, focusFn(false));
      } else {
        before = await takeSnapshot(tabId, t);
      }
      await keys(tabId, chords);
      extra = keyLabel(args.keys ?? args.text);
      break;
    }
    case "select": {
      if (!hasTarget) fail("bad_args", "select needs a ref or a locator");
      const value = args.value ?? args.text;
      if (value === undefined) fail("bad_args", "select needs a value (the option's value or label)");
      const r = await need({ visible: true, enabled: true });
      ({ entry: el, snap: before } = r);
      await cursor(tabId, agent, r.point, true);
      const res = (await callOn(tabId, el.backendId, selectFn, [{ value }])) as { error?: string; options?: string[] } | undefined;
      if (res?.error === "not_select") fail("bad_args", `${el.line} is not a <select>; click it, then click the option`, { reason: "it isn't a list" });
      if (res?.error === "no_option") fail("not_found", `no option "${value}" in ${el.line}`, { candidates: res.options });
      break;
    }
    case "check": {
      if (!hasTarget) fail("bad_args", "check needs a ref or a locator");
      const want = args.value !== false;
      const r = await need({ visible: true, enabled: true });
      ({ entry: el, snap: before } = r);
      if (el.checked !== want) {
        await cursor(tabId, agent, r.point, true);
        await mouse(tabId, r.point.x, r.point.y, "click");
        await sleep(50);
        const now = (await takeSnapshot(tabId, t)).entries.find((e) => e.backendId === el!.backendId);
        if (now && now.checked !== want) fail("timeout", `clicking ${el.line} didn't ${want ? "check" : "uncheck"} it`, { reason: "it didn't change" });
      }
      break;
    }
    case "scroll": {
      let node: number | undefined | null = null, point: { x: number; y: number } | null = null;
      if (hasTarget) {
        const r = await need({ visible: !args.direction ? false : true });
        ({ entry: el, snap: before } = r);
        node = el.backendId;
        point = r.point;
      } else {
        before = await takeSnapshot(tabId, t);
      }
      if (node && !args.direction && args.amount === undefined) {
        await callOn(tabId, node, function (this: Node) { (this.nodeType === 1 ? (this as Element) : this.parentElement!).scrollIntoView({ block: "center", inline: "nearest" }); });
        break;
      }
      const dir = args.direction ?? "down";
      if (!["up", "down", "left", "right"].includes(dir)) fail("bad_args", `direction is up, down, left or right, not ${JSON.stringify(dir)}`);
      type Size = { w: number; h: number; sig: string };
      const size = (node ? await callOn(tabId, node, viewSize) : await evaluate(tabId, `(${viewSize})()`)) as Size;
      const amount = Number.isFinite(args.amount) ? args.amount! : Math.round(0.8 * (dir === "left" || dir === "right" ? size.w : size.h));
      const [dx, dy] = ({ up: [0, -amount], down: [0, amount], left: [-amount, 0], right: [amount, 0] } as Record<string, number[]>)[dir];
      const { cssVisualViewport: vv } = await cdp<Viewport>(tabId, "Page.getLayoutMetrics");
      const x = point ? point.x : Math.round(vv.clientWidth / 2), y = point ? point.y : Math.round(vv.clientHeight / 2);
      await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseWheel", x, y, deltaX: dx, deltaY: dy });
      await sleep(80);
      const after = (node ? await callOn(tabId, node, viewSize) : await evaluate(tabId, `(${viewSize})()`)) as Size;
      if (after.sig === size.sig) {
        // The wheel didn't move it (some pages, hidden tabs): scroll from script.
        if (node) await callOn(tabId, node, scrollFn, [{ value: dx }, { value: dy }]);
        else await evaluate(tabId, `(${scrollFn}).call(null, ${dx}, ${dy})`);
      }
      break;
    }
    case "goto": {
      const url = normalUrl(args.url);
      refuseUrl("goto", url);
      before = await takeSnapshot(tabId, t);
      const r = await cdp<{ errorText?: string }>(tabId, "Page.navigate", { url });
      if (r.errorText) fail("bad_args", `couldn't load ${url}: ${r.errorText}`, { summary: failure("goto", hostOf(url), "bad_args", r.errorText) });
      await sleep(50);
      await waitLoaded(tabId, Math.max(timeout, 15_000));
      break;
    }
    case "close": {
      const tab = await chrome.tabs.get(tabId);
      const a = agentOf(agent);
      // Its last tab: Chrome removes the group next. Remember which group
      // (not a timed flag: a close right after another agent-closed group
      // raced the timer and read as the user closing the group).
      if (tabsOf(agent).length === 1) a.closingGroup = a.groupId;
      await detach(tabId, t);
      forget(tabId);
      await chrome.tabs.remove(tabId);
      return { ok: true, url: tab.url, title: tab.title, changed: "", summary: summary("close", null, args, hostOf(tab.url)) };
    }
    case "wait": {
      if (hasTarget) {
        const r = await need({ visible: true });
        el = r.entry;
      } else if (args.text !== undefined) {
        const want = String(args.text).toLowerCase();
        for (;;) {
          const body = String((await evaluate(tabId, "document.body ? document.body.innerText : ''")) || "").toLowerCase();
          if (body.includes(want)) break;
          if (Date.now() >= deadline) fail("timeout", `the page doesn't show "${args.text}" after ${timeout} ms`, { reason: "it never showed up" });
          await sleep(150);
        }
      } else {
        await sleep(Math.min(Math.max(args.amount ?? 0, 0), timeout));
      }
      const tab = await chrome.tabs.get(tabId);
      return { ok: true, url: tab.url, title: tab.title, changed: "", summary: summary("wait", el, args, hostOf(tab.url)) };
    }
    case "set_input_files": {
      const up = await setInputFiles(tabId, t, agent, args, deadline, seen);
      ({ el, before } = up);
      more = { via: up.via };
      break;
    }
    case "expect_chooser": {
      await expectChooser(tabId, t);
      const tab = await chrome.tabs.get(tabId);
      return { ok: true, url: tab.url, title: tab.title, changed: "", summary: summary("expect_chooser", null, args, hostOf(tab.url)) };
    }
    case "wait_chooser": {
      const c = await waitChooser(tabId, t, timeout);
      const tab = await chrome.tabs.get(tabId);
      return { ok: true, url: tab.url, title: tab.title, changed: "", summary: summary("wait_chooser", null, args, hostOf(tab.url)), ...c };
    }
    case "read": {
      let text;
      if (hasTarget) {
        const r = await need({ visible: false });
        el = r.entry;
        text = await callOn(tabId, el.backendId, readFn);
      } else {
        text = await evaluate(tabId, "document.body ? document.body.innerText : ''");
      }
      text = String(text ?? "");
      if (!hasTarget && !text.trim() && (await isPdf(tabId))) text = PDF_NOTE.slice(2);
      if (text.length > 4000) text = text.slice(0, 4000) + "…";
      const tab = await chrome.tabs.get(tabId);
      return { ok: true, url: tab.url, title: tab.title, changed: text, summary: summary("read", el, args, hostOf(tab.url)) };
    }
  }

  const t1 = Date.now();
  const after = await settle(tabId, t, deadline);
  note({ act: action, actMs: t1 - started, settleMs: Date.now() - t1 });
  return {
    ok: true,
    url: after.url,
    title: after.title,
    changed: changedText(before!, after),
    summary: summary(action, el, args, after.host, extra),
    ...more,
  };
}

// ---------------------------------------------------------------- screenshot

type Clip = { x: number; y: number; width: number; height: number };

async function screenshotOp(agent: string, args: ActArgs) {
  const { tabId, t } = await owned(agent, args.target);
  return onTab(agent, tabId, t, async () => {
    await attach(tabId, t);
    let clip: Clip | null = null;
    if (args.ref || args.locator) {
      // (visible: resolve gives its point)
      const r = await resolve(tabId, t, args, Date.now() + (args.timeout_ms ?? 5000), { visible: true });
      const b = r.point!.box;
      const after = (await cdp<Viewport>(tabId, "Page.getLayoutMetrics")).cssVisualViewport;
      const x0 = Math.max(b.x, 0), y0 = Math.max(b.y, 0);
      const x1 = Math.min(b.x + b.width, after.clientWidth), y1 = Math.min(b.y + b.height, after.clientHeight);
      clip = { x: x0 + after.pageX, y: y0 + after.pageY, width: x1 - x0, height: y1 - y0 };
    }
    return capture(tabId, clip, args.max_width);
  });
}

/** The capture both screenshot and peek use: `clip` (page CSS px) or the viewport, at most `maxWidth` px wide. */
async function capture(tabId: number, clip: Clip | null, maxWidth: number | undefined) {
  const maxW = Math.min(Math.max(Number.isFinite(maxWidth) ? maxWidth! : 1280, 64), 4096);
  const dpr = ((await evaluate(tabId, "devicePixelRatio")) as number) || 1;
  if (!clip) {
    const { cssVisualViewport: vv } = await cdp<Viewport>(tabId, "Page.getLayoutMetrics");
    clip = { x: vv.pageX, y: vv.pageY, width: vv.clientWidth, height: vv.clientHeight };
  }
  const scale = Math.min(1, maxW / (clip.width * dpr)) * 1;
  const shot = await cdp<{ data: string }>(tabId, "Page.captureScreenshot", { format: "jpeg", quality: 80, clip: { ...clip, scale }, captureBeyondViewport: false });
  const { width, height } = jpegSize(shot.data);
  return { data: shot.data, mime: "image/jpeg", width, height };
}

/**
 * C4 `peek` (ctl `peek`, docs/issues/18 step 5): a still of the agent's
 * tab for the user's window. Only while its debugger is attached (it
 * never attaches to look); quiet: no cursor, no overlay, not in the tab's
 * queue of actions, nothing the agent's next diff or the user's touch
 * marks see.
 */
async function peekOp(agent: string, args: ActArgs) {
  const m = /^tab:(\d+)$/.exec(String(args.target ?? ""));
  const tabId = m ? Number(m[1]) : NaN;
  const t = tabs.get(tabId);
  if (!t || t.agent !== agent || !t.attached || t.paused || agents.get(agent)?.stopped) fail("not_found", "this agent drives no tab now");
  try {
    return await capture(tabId, null, args.max_width);
  } catch {
    fail("not_found", "the tab let go meanwhile");
  }
}

export { ensureOverlay, touched, act, screenshotOp, peekOp, resolve, pointOf, mouse, cursor };
