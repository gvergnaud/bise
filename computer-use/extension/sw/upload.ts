// Uploads (task cu-upload): attaching files to a page without ever a
// native dialog. Every agent tab is attached with file choosers
// intercepted (cdp.ts attach): a click that opens one only fires
// Page.fileChooserOpened, recorded here. Files go by path: the browser
// reads them from disk (DOM.setFileInputFiles, a CDP drop with files);
// no byte crosses native messaging. The broker checked the paths
// (rust/computer-use/src/files.rs) and sends their real paths.
//
// set_input_files, on a locator, a ref or a chooser, in this order:
//   1. a chooser (waitForFileChooser): its input, or the parked
//      showOpenFilePicker() call (the shim below);
//   2. the element is an <input type=file>, or leads to exactly one (its
//      label's control, aria-controls, the one inside it, the one input
//      of the nearest ancestor holding only one): DOM.setFileInputFiles;
//   3. a click on it, with the shim on, opens a chooser within 2 s: fed;
//   4. a drop: Input.dispatchDragEvent with the files (trusted), only
//      when the page accepts the dragover (else Chrome would open the
//      file in the tab); not accepted, a DataTransfer built in the page
//      from a temporary input's files (untrusted events);
//   5. else no_file_input, with candidates.

import { fail, sleep } from "./state.js";
import { cdp, callOn, takeSnapshot } from "./cdp.js";
import { resolve, pointOf, mouse, cursor } from "./act.js";
import { filesOf, nextChooser, sawPicker, remember, uploadCandidates } from "../lib/upload.js";
import type { Chooser } from "../lib/upload.js";
import type { Tab } from "./state.js";
import type { Entry } from "../lib/ax.js";
import type { Evaluated, RemoteObject } from "./cdp.js";
import type { ActArgs } from "./act.js";

/** What set_input_files did: for doAct's diff and result. */
export type Uploaded = { el: Entry | null; before: Awaited<ReturnType<typeof takeSnapshot>>; via: "input" | "chooser" | "drop" };

// ---------------------------------------------------------------- choosers

/** Page.fileChooserOpened (cdp.ts's listener): one more chooser on the tab. */
export function chooserOpened(t: Tab, params: { mode?: string; backendNodeId?: number }) {
  const c: Chooser = { id: ++t.chooserSeq, kind: params.backendNodeId ? "input" : "picker", multiple: params.mode === "selectMultiple" };
  if (params.backendNodeId) c.backendNodeId = params.backendNodeId;
  t.choosers = remember(t.choosers, c);
}

// The shim: showOpenFilePicker() parks its call in the page until bise
// feeds it (set_input_files with that chooser) or lets go (unshim: an
// AbortError, as a cancelled picker gives). The key is not enumerable.
const SHIM = `(() => {
  const k = "__biseFilePicker";
  if (window[k]) return true;
  const st = { orig: window.showOpenFilePicker, seq: 0, calls: [] };
  Object.defineProperty(window, k, { value: st, configurable: true, enumerable: false });
  window.showOpenFilePicker = function (opts) {
    return new Promise((resolve, reject) => st.calls.push({ id: ++st.seq, multiple: !!(opts && opts.multiple), resolve, reject, taken: false }));
  };
  return true;
})()`;
const UNSHIM = `(() => {
  const k = "__biseFilePicker", st = window[k];
  if (!st) return false;
  if (st.orig) window.showOpenFilePicker = st.orig; else delete window.showOpenFilePicker;
  for (const c of st.calls) if (!c.done) { c.done = true; c.reject(new DOMException("The user aborted a request.", "AbortError")); }
  delete window[k];
  return true;
})()`;
// the parked calls bise has not handed out yet
const PARKED = `(() => {
  const st = window.__biseFilePicker;
  if (!st) return [];
  const out = st.calls.filter((c) => !c.taken && !c.done).map((c) => ({ id: c.id, multiple: c.multiple }));
  for (const c of st.calls) c.taken = true;
  return out;
})()`;

async function evalValue(tabId: number, expression: string): Promise<unknown> {
  const r = await cdp<Evaluated>(tabId, "Runtime.evaluate", { expression, returnByValue: true });
  return r.result?.value;
}

async function shim(tabId: number, t: Tab) {
  await evalValue(tabId, SHIM);
  t.shimmed = true;
}

/** Let go of the shim (after an upload, on an error, before a detach): a parked call gets an AbortError. */
export async function unshim(tabId: number, t: Tab) {
  if (!t.shimmed) return;
  t.shimmed = false;
  await evalValue(tabId, UNSHIM).catch(() => {});
}

/** The page's parked picker calls become choosers of the tab. */
async function pollParked(tabId: number, t: Tab) {
  if (!t.shimmed) return;
  const parked = ((await evalValue(tabId, PARKED).catch(() => [])) as { id: number; multiple: boolean }[]) || [];
  for (const p of parked) t.choosers = remember(t.choosers, { id: ++t.chooserSeq, kind: "fsa", fsa: p.id, multiple: p.multiple });
}

/** A chooser opened after `from`, within `ms`; null when none. */
async function chooserAfter(tabId: number, t: Tab, from: number, ms: number): Promise<Chooser | null> {
  const until = Date.now() + ms;
  for (;;) {
    await pollParked(tabId, t);
    const c = nextChooser(t.choosers, from);
    if (c || Date.now() >= until) return c;
    await sleep(80);
  }
}

/** C1 `expect_chooser`: forget the past choosers, shim the picker; the next one opened is waitForFileChooser's. */
export async function expectChooser(tabId: number, t: Tab) {
  for (const c of t.choosers) c.used = true;
  await shim(tabId, t);
  t.chooserFrom = t.chooserSeq;
}

/** C1 `wait_chooser`: the chooser opened since expect_chooser, within the timeout. */
export async function waitChooser(tabId: number, t: Tab, timeout: number) {
  const from = (t.chooserFrom as number | undefined) ?? t.chooserSeq;
  const c = await chooserAfter(tabId, t, from, timeout);
  if (!c) {
    const why = sawPicker(t.choosers, from)
      ? "the page opened a file picker bise can't feed (a picker it started before bise was ready); call waitForFileChooser again around the click"
      : `no file chooser opened within ${timeout} ms; snapshot to see what the click did, or use setInputFiles on the upload button`;
    fail("chooser_not_opened", why, { reason: "no file chooser opened" });
  }
  const got = c as Chooser;
  return { chooser: got.id, multiple: got.multiple, kind: got.kind === "fsa" ? "picker" : "input" };
}

// ---------------------------------------------------------------- the input of an element

// in the page: `this` is the element; the <input type=file> it leads to, or null
function inputOfFn(this: Node) {
  const el = (this.nodeType === 1 ? this : this.parentElement) as HTMLElement | null;
  if (!el) return null;
  const isFile = (e: Element | null | undefined): e is HTMLInputElement => !!e && e.tagName === "INPUT" && (e as HTMLInputElement).type === "file";
  if (isFile(el)) return el;
  const lab = (el.tagName === "LABEL" ? el : el.closest("label")) as HTMLLabelElement | null;
  if (lab && isFile(lab.control)) return lab.control;
  for (const id of (el.getAttribute("aria-controls") || "").split(/\s+/).filter(Boolean)) {
    const c = document.getElementById(id);
    if (isFile(c)) return c;
  }
  const inside = el.querySelectorAll("input[type=file]");
  if (inside.length === 1) return inside[0];
  if (inside.length > 1) return null;
  let p = el.parentElement;
  for (let i = 0; i < 4 && p && p !== document.body; i++, p = p.parentElement) {
    const all = p.querySelectorAll("input[type=file]");
    if (all.length === 1) return all[0];
    if (all.length > 1) return null;
  }
  return null;
}

// in the page: the file inputs, as a no_file_input names them
function fileInputsFn() {
  return [...document.querySelectorAll("input[type=file]")].slice(0, 10).map((e) => {
    const i = e as HTMLInputElement;
    const name = i.labels?.[0]?.innerText || i.getAttribute("aria-label") || i.name || i.id || "(no name)";
    const r = i.getBoundingClientRect();
    return { name: name.replace(/\s+/g, " ").trim().slice(0, 60), hidden: r.width < 2 || r.height < 2 || getComputedStyle(i).visibility === "hidden" };
  });
}

/** The backend node of an element a page function returns (null: none). */
async function nodeOf(tabId: number, backendNodeId: number, fn: Function): Promise<number | null> {
  const { object } = await cdp<{ object: RemoteObject }>(tabId, "DOM.resolveNode", { backendNodeId, objectGroup: "bise-up" });
  try {
    const r = await cdp<Evaluated>(tabId, "Runtime.callFunctionOn", { objectId: object.objectId, functionDeclaration: fn.toString(), returnByValue: false });
    if (r.exceptionDetails || !r.result?.objectId || r.result.subtype !== "node") return null;
    const { node } = await cdp<{ node: { backendNodeId: number } }>(tabId, "DOM.describeNode", { objectId: r.result.objectId });
    return node.backendNodeId;
  } finally {
    cdp(tabId, "Runtime.releaseObjectGroup", { objectGroup: "bise-up" }).catch(() => {});
  }
}

async function feedInput(tabId: number, input: number, files: string[]) {
  const info = (await callOn(tabId, input, function (this: HTMLInputElement) { return { multiple: this.multiple, disabled: this.disabled }; })) as { multiple: boolean; disabled: boolean };
  if (info.disabled) fail("timeout", "the file input is disabled", { reason: "it's disabled" });
  if (files.length > 1 && !info.multiple) fail("bad_args", `this input takes one file and you gave ${files.length}; attach them one per slot`, { reason: "it takes one file" });
  await cdp(tabId, "DOM.setFileInputFiles", { files, backendNodeId: input });
}

// ---------------------------------------------------------------- a temporary input (the picker, the page-side drop)

/** Runs `fn` in the page on a temporary hidden input holding `files` (read by the browser from disk), then removes it. */
async function withTempFiles(tabId: number, files: string[], fn: Function, args: object[] = []): Promise<unknown> {
  const made = await cdp<Evaluated>(tabId, "Runtime.evaluate", {
    expression: `(() => { const i = document.createElement("input"); i.type = "file"; i.multiple = true; i.hidden = true; i.setAttribute("aria-hidden", "true"); (document.body || document.documentElement).appendChild(i); return i; })()`,
    objectGroup: "bise-tmp",
  });
  try {
    if (!made.result?.objectId) fail("no_file_input", "couldn't make a file input in this page");
    const { node } = await cdp<{ node: { backendNodeId: number } }>(tabId, "DOM.describeNode", { objectId: made.result!.objectId });
    await cdp(tabId, "DOM.setFileInputFiles", { files, backendNodeId: node.backendNodeId });
    const r = await cdp<Evaluated>(tabId, "Runtime.callFunctionOn", { objectId: made.result!.objectId, functionDeclaration: fn.toString(), arguments: args, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    return r.result?.value;
  } finally {
    if (made.result?.objectId) await cdp(tabId, "Runtime.callFunctionOn", { objectId: made.result.objectId, functionDeclaration: "function () { this.remove(); }" }).catch(() => {});
    cdp(tabId, "Runtime.releaseObjectGroup", { objectGroup: "bise-tmp" }).catch(() => {});
  }
}

// in the page, on the temporary input: hand its files to the parked picker call
function feedPickerFn(this: HTMLInputElement, id: number) {
  const st = (window as unknown as { __biseFilePicker?: { calls: { id: number; done?: boolean; resolve: (v: unknown) => void }[] } }).__biseFilePicker;
  const call = st?.calls.find((c) => c.id === id && !c.done);
  if (!call) return false;
  const handles = [...(this.files || [])].map((f) => {
    const h = {
      kind: "file",
      name: f.name,
      getFile: async () => f,
      isSameEntry: async (o: unknown) => o === h,
      queryPermission: async () => "granted",
      requestPermission: async () => "granted",
    };
    return h;
  });
  call.done = true;
  call.resolve(handles);
  return true;
}

// in the page, on the temporary input: drop its files on the element `el`;
// true when the page took them (its dragover said yes)
function dropFn(this: HTMLInputElement, el: Element) {
  const dt = new DataTransfer();
  for (const f of this.files || []) dt.items.add(f);
  const r = el.getBoundingClientRect();
  const at = { clientX: r.left + r.width / 2, clientY: r.top + r.height / 2, bubbles: true, cancelable: true, composed: true, dataTransfer: dt };
  el.dispatchEvent(new DragEvent("dragenter", at));
  const over = new DragEvent("dragover", at);
  el.dispatchEvent(over);
  if (!over.defaultPrevented) {
    el.dispatchEvent(new DragEvent("dragleave", at));
    return false;
  }
  el.dispatchEvent(new DragEvent("drop", at));
  return true;
}

// ---------------------------------------------------------------- the drop

// in the page: watch the next drag (the page's own answer to dragover, a
// drop that would leave the page). The listeners are on window, the
// first ones: they only read, but the last stops a drop no one took.
const WATCH = `(() => {
  const w = { over: null, drops: 0 };
  const onOver = (e) => { w.over = e; };
  const onDrop = () => { w.drops++; };
  const guard = (e) => { e.preventDefault(); };
  addEventListener("dragover", onOver, true);
  addEventListener("drop", onDrop, true);
  addEventListener("drop", guard, false);
  w.done = () => { removeEventListener("dragover", onOver, true); removeEventListener("drop", onDrop, true); removeEventListener("drop", guard, false); };
  Object.defineProperty(window, "__biseDrag", { value: w, configurable: true, enumerable: false });
  return true;
})()`;
const ACCEPTED = `!!(window.__biseDrag && window.__biseDrag.over && window.__biseDrag.over.defaultPrevented)`;
const DROPS = `(() => { const w = window.__biseDrag; if (!w) return 0; w.done(); delete window.__biseDrag; return w.drops; })()`;

/** A trusted drop of `files` at the point; true when the page took it. */
async function cdpDrop(tabId: number, x: number, y: number, files: string[]): Promise<boolean> {
  const data = { items: [], files, dragOperationsMask: 1 };
  await evalValue(tabId, WATCH);
  try {
    await cdp(tabId, "Input.dispatchDragEvent", { type: "dragEnter", x, y, data });
    await cdp(tabId, "Input.dispatchDragEvent", { type: "dragOver", x, y, data });
    if (!(await evalValue(tabId, ACCEPTED))) {
      await cdp(tabId, "Input.dispatchDragEvent", { type: "dragCancel", x, y, data }).catch(() => {});
      return false;
    }
    await cdp(tabId, "Input.dispatchDragEvent", { type: "drop", x, y, data });
    await sleep(50);
    return true;
  } finally {
    await evalValue(tabId, DROPS).catch(() => 0);
  }
}

/** The page-side drop (untrusted events) on the element: true when the page took it. */
async function pageDrop(tabId: number, backendNodeId: number, files: string[]): Promise<boolean> {
  const { object } = await cdp<{ object: RemoteObject }>(tabId, "DOM.resolveNode", { backendNodeId, objectGroup: "bise-tmp" });
  return !!(await withTempFiles(tabId, files, dropFn, [{ objectId: object.objectId }]));
}

// ---------------------------------------------------------------- set_input_files

/** Feed a chooser: its input, or its parked picker call. */
async function feedChooser(tabId: number, c: Chooser, files: string[]) {
  if (files.length > 1 && !c.multiple) fail("bad_args", `this chooser takes one file and you gave ${files.length}; attach them one per slot`, { reason: "it takes one file" });
  c.used = true;
  if (c.kind === "fsa") {
    const ok = await withTempFiles(tabId, files, feedPickerFn, [{ value: c.fsa }]);
    if (!ok) fail("chooser_not_opened", "the page's file picker was closed meanwhile; open it again with waitForFileChooser");
  } else {
    await cdp(tabId, "DOM.setFileInputFiles", { files, backendNodeId: c.backendNodeId });
  }
}

/**
 * C1 `set_input_files`: the files on the element (or chooser) of `args`,
 * by the first way that works (the order in this file's head).
 */
export async function setInputFiles(tabId: number, t: Tab, agent: string, args: ActArgs, deadline: number, seen: (e: Entry) => void): Promise<Uploaded> {
  const files = filesOf(args.files);
  if (typeof files === "string") fail("bad_args", files);
  const list = files as string[];
  try {
    if (args.chooser !== undefined && args.chooser !== null) {
      const before = await takeSnapshot(tabId, t);
      await pollParked(tabId, t);
      const c = t.choosers.find((x) => x.id === Number(args.chooser));
      if (!c || c.used || c.kind === "picker") fail("chooser_not_opened", `chooser ${args.chooser} isn't open (used, or too old); call waitForFileChooser again`);
      await feedChooser(tabId, c!, list);
      return { el: null, before, via: "chooser" };
    }
    if (!args.ref && !args.locator) fail("bad_args", "set_input_files needs a ref, a locator or a chooser");
    // hidden inputs are fine: no point needed to find it
    const r = await resolve(tabId, t, args, deadline, { visible: false });
    seen(r.entry);
    const { entry: el, snap: before } = r;
    const input = await nodeOf(tabId, el.backendId!, inputOfFn);
    if (input) {
      await feedInput(tabId, input, list);
      return { el, before, via: "input" };
    }
    const pt = await pointOf(tabId, el.backendId);
    if (pt.ok) {
      // a button that opens a chooser: intercepted, never a dialog
      const from = t.chooserSeq;
      await shim(tabId, t);
      await cursor(tabId, agent, pt, true);
      await mouse(tabId, pt.x, pt.y, "click");
      const c = await chooserAfter(tabId, t, from, Math.min(2000, Math.max(deadline - Date.now(), 800)));
      if (c) {
        await feedChooser(tabId, c, list);
        return { el, before, via: "chooser" };
      }
      // a drop zone: a trusted drop where the page takes it
      if (await cdpDrop(tabId, pt.x, pt.y, list)) return { el, before, via: "drop" };
    }
    if (await pageDrop(tabId, el.backendId!, list)) return { el, before, via: "drop" };
    const inputs = (await evalValue(tabId, `(${fileInputsFn})()`)) as { name: string; hidden: boolean }[];
    fail("no_file_input", `${el.line} has no file input, opened no file chooser and took no drop; target the upload button or the input (see candidates)`, {
      candidates: uploadCandidates(inputs || [], before.entries),
      reason: "it takes no file",
    });
  } finally {
    await unshim(tabId, t);
  }
  throw new Error("unreachable");
}
