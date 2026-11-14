// node --experimental-strip-types --test dev/plugins/ambient-qa/as-he-does/logpoint.test.ts
import test from 'node:test';
import assert from 'node:assert/strict';
import { findFn, noLogpoint, pickLogpoint, type ParsedScript } from './logpoint.ts';

// the preload as built since 789a36c1e (openExternal takes opts), its api one line lower than
// before, and the web tab's own preload first in the parse order (no openExternal in it)
const WEB_PRELOAD = [
  '"use strict";',
  'const electron = require("electron");',
  'electron.contextBridge.exposeInMainWorld("webTab", { back: () => electron.ipcRenderer.send("web:back") });',
].join('\n');
const PRELOAD = [
  '"use strict";',
  'const electron = require("electron");',
  'const IPC = { openExternal: "bise:open-external", quickLook: "bise:quick-look" };',
  'const api = {',
  '  web: { out: async (url) => electron.ipcRenderer.invoke("bise:web-out", url) },',
  '  openExternal: async (url, opts) => (await electron.ipcRenderer.invoke(IPC.openExternal, url, opts)) === true,',
  '  quickLook: async (path) => { await electron.ipcRenderer.invoke(IPC.quickLook, path); },',
  '  async showInFinder(p) { await electron.ipcRenderer.invoke("bise:show-in-finder", p); },',
  '};',
].join('\n');
const scripts: ParsedScript[] = [
  { scriptId: '7', url: 'node:electron/js2c/sandbox_bundle', startLine: 0, startColumn: 0, source: 'module.exports = {};' },
  { scriptId: '9', url: 'file:///app/out/preload/web.js', startLine: 0, startColumn: 0, source: WEB_PRELOAD },
  { scriptId: '11', url: 'file:///app/out/preload/index.js', startLine: 0, startColumn: 0, source: PRELOAD },
];

test("openExternal(url, opts): the logpoint lands on its body in the app's preload, not the web tab's", () => {
  const lp = pickLogpoint(scripts, 'openExternal');
  assert.ok(lp, 'a logpoint');
  assert.equal(lp.scriptId, '11');
  assert.equal(lp.param, 'url');
  assert.equal(lp.lineNumber, 5);
  const line = PRELOAD.split('\n')[lp.lineNumber];
  assert.ok(line.slice(lp.columnNumber).startsWith('(await electron.ipcRenderer.invoke(IPC.openExternal'), line.slice(lp.columnNumber));
});

test("a script wrapped at an offset: its startLine moves the line, its startColumn only the first line's", () => {
  const lp = pickLogpoint([{ ...scripts[2], startLine: 40, startColumn: 12 }], 'openExternal');
  assert.deepEqual([lp?.lineNumber, lp?.columnNumber], [45, PRELOAD.split('\n')[5].indexOf('(await')]);
  const one = pickLogpoint([{ scriptId: '1', url: 'u', startLine: 3, startColumn: 10, source: 'x={openExternal:async u=>go(u)}' }], 'openExternal');
  assert.deepEqual([one?.lineNumber, one?.columnNumber, one?.param], [3, 10 + 'x={openExternal:async u=>'.length, 'u']);
});

test('every form the preload could take names its first parameter', () => {
  assert.equal(findFn('openExternal: async (url) => 1', 'openExternal')?.param, 'url');
  assert.equal(findFn('openExternal: (url, { background } = {}) => 1', 'openExternal')?.param, 'url');
  assert.equal(findFn('openExternal: async function (u, o) { return 1 }', 'openExternal')?.param, 'u');
  assert.equal(findFn(PRELOAD, 'quickLook')?.param, 'path');
  assert.equal(findFn(PRELOAD, 'showInFinder')?.param, 'p');
  // a call or the IPC name is no definition
  assert.equal(findFn('window.bise.openExternal(url); const IPC = { openExternal: "bise:x" };', 'openExternal'), null);
});

test('no preload with it: no logpoint, and the reason names the scripts seen', () => {
  assert.equal(pickLogpoint(scripts.slice(0, 2), 'openExternal'), null);
  assert.match(noLogpoint(scripts.slice(0, 2), 'openExternal'), /no script defines openExternal among 2: .*preload\/web\.js/);
});
