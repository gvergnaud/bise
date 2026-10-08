// bise computer use: the service worker.
//
// It talks to bise over native messaging (contract C4 of
// docs/computer-use-briefs.md): bise asks for open / tabs / snapshot /
// screenshot / act on behalf of an agent; each agent works in its own tab
// group "bise · <agent>", in background tabs, through chrome.debugger (CDP).
// The user stays in control: the debugging bar's Cancel or closing the group
// stops the agent; touching one of its tabs pauses it.
//
// The code is in sw/, by area: state.js (the agents, their tabs, the
// groups' titles), host.js (the native port), tabs.js (the ops, open,
// show), cdp.js (the debugger, the snapshot), act.js (driving a tab),
// control.js (stop, pause, drop, the user's moves). This file starts it.

import { agents, tabs, log, note } from "./sw/state.js";
import { port, timer, connect } from "./sw/host.js";
import { detached } from "./sw/cdp.js";
import "./sw/control.js";
import { BUILD } from "./build.js";

chrome.runtime.onStartup.addListener(() => {});
chrome.runtime.onInstalled.addListener(() => {});

// MV3 may stop this worker (the open native port keeps it alive, but an
// update, memory pressure or a crash still stop it), and bise can't wake
// it: the port is ours to open. An alarm every 30 s (the shortest Chrome
// allows) starts it again; starting runs connect() below. The broker
// waits for that hello instead of saying no_browser (WAKE_WAIT).
chrome.alarms.create("bise-wake", { periodInMinutes: 0.5 });
chrome.alarms.onAlarm.addListener(() => {
  checkBuild();
  if (!port && !timer) connect();
});

// A loaded-unpacked extension never picks up new files by itself: Chrome
// kept running the first build while bise moved on (launch's re-test: the
// per-letter snapshot fix and {css} -> bad_args never reached Chrome).
// bise syncs each version into ~/.bise/computer-use/extension with a new
// build id in build.js (what this code was loaded with) and build.json
// (read from disk now): when they differ, reload. Never in the middle of
// an action: the next alarm tries again.
async function checkBuild() {
  try {
    const onDisk = (await (await fetch(chrome.runtime.getURL("build.json"), { cache: "no-store" })).json()).build;
    const busy = [...tabs.values()].some((t) => t.acting);
    if (onDisk && onDisk !== BUILD && !busy) {
      note({ reload: { from: BUILD, to: onDisk } });
      chrome.runtime.reload();
    }
  } catch {
    // no build.json (an old copy): nothing to compare
  }
}
checkBuild();

// For the tests (test/e2e.mjs reads it through CDP on this worker).
globalThis.bise = { agents, tabs, log, connected: () => !!port, detached, build: BUILD, checkBuild };

connect();
