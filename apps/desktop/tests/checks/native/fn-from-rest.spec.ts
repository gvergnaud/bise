// native/fn-from-rest · bar O.5: fn held from rest, with another app in front (the old check: listens
// in under 300 ms; the app doesn't steal focus). 'Rest': the app launched 2 s ago, nothing shown, the
// helper says another app is in front (TextEdit). Passes when the core gets talk_start within 300 ms
// of the helper's talk_start, the talk shows in the corner without the keys, no window of bise is
// focused, and the window server never saw bise frontmost (the proof's watch, whole run). The real
// fn key and TextEdit staying active on a real screen are the VM pass's (vm/vm-pass.spec.ts: green
// on 442f748a).
// The 300 ms bar is an idle Mac's: on a busy one it stretches by the load (harness/load.ts, as
// tests/wait.py's waits: 366 ms at load ~15 in 11.11's pass, main m_64400).
import { test, expect, loadFactor } from '../../harness';
import { CAPSULE, onWorld, sleep } from './world';

test('native/fn-from-rest: fn held with another app in front listens within 300 ms, no focus taken', async () => {
  await onWorld(async ({ s, helper, core, out }) => {
    await sleep(2000);
    expect(out.of(CAPSULE), 'at rest: nothing shown').toEqual([]);
    await helper.send({ ev: 'front', app: 'TextEdit', bundle: 'com.apple.TextEdit', title: 'note.txt', url: null, path: '/Users/camille/note.txt' });
    await sleep(300);
    const before = core.cmds().length;
    const t0 = Date.now();
    await helper.send({ ev: 'fn', act: 'talk_start', by: 'fn' });
    await core.waitCmd((c) => c.cmd === 'talk_start', 1000);
    const ms = Date.now() - t0;
    const load = loadFactor();
    const bar = Math.round(300 * load);
    expect(ms, `the core listens ${String(ms)} ms after fn (under ${String(bar)}: 300 at load x${load.toFixed(1)})`).toBeLessThan(bar);
    expect(core.cmds().slice(before).filter((c) => c.cmd === 'talk_start').length, 'one talk_start').toBe(1);
    expect(await out.wait(CAPSULE, (l) => l.startsWith('show')), 'the talk in the corner, never the keys').toBe('show corner');
    const focused = await s.app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().filter((b) => b.isFocused()).length);
    expect(focused, 'no window of bise is focused').toBe(0);
    await helper.send({ ev: 'fn', act: 'talk_end' });
    await core.waitCmd((c) => c.cmd === 'talk_end', 1000);
    expect(s.watch.violations, 'the window server never saw bise in front or on screen').toEqual([]);
  });
});
