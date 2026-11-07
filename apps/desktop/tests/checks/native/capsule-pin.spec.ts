// native/capsule-pin · his try's item 6 ('la fenêtre bouge un peu'): the capsule is placed once,
// at show, then pinned (amb-mac fa3bad10 + 2887644d, place.ts Pin / fitBounds). On the scripted
// helper and core, off-screen (place.ts's OFF_DISPLAY: 1920x1200 at -30000,-30000, no Dock: the
// work area's bottom stands for the Dock line). The frame is the capsule window's bounds, read in
// Electron's main after each step; the glass is the drawn panel in the capsule's page.
//   (a) fn space, then 20 real keystrokes that filter the list: the frame's top-left never moves
//       while its size follows the list
//   (b) a voice session (fn held, then its release) in the corner card: the glass's bottom-right
//       never moves (it grows upward), sits 16 px above the Dock line and never passes it
//   ((c), a pushed card's place, is gone with the push: b3, the user's decision 2026-10-09)
import type { Page } from '@playwright/test';
import { test, expect } from '../../harness';
import { CAPSULE, capsulePage, capsuleWindow, onWorld, sleep, type World } from './world';

const DISPLAY = { x: -30000, y: -30000, width: 1920, height: 1200 };
const DOCK_LINE = DISPLAY.y + DISPLAY.height;
/** the corner card's margin (place.ts CORNER) */
const CORNER = 16;

type Frame = { x: number; y: number; width: number; height: number };
const frame = async (w: World): Promise<Frame> => {
  const { x, y, width, height } = await capsuleWindow(w.s);
  return { x, y, width, height };
};

/** the glass (a data-glass panel of the view) in screen coordinates */
async function glass(w: World, cap: Page, id: string): Promise<Frame> {
  const g = await cap.getByTestId(id).evaluate((e) => {
    const r = e.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
  });
  const f = await frame(w);
  return { x: f.x + g.x, y: f.y + g.y, width: g.width, height: g.height };
}

const sizes = (fs: readonly Frame[]): string[] => [...new Set(fs.map((f) => `${f.width}x${f.height}`))];

test('native/capsule-pin (a): fn space, 20 keystrokes filter the list, the frame top-left never moves', async () => {
  await onWorld(async (w) => {
    await w.helper.send({ ev: 'fn', act: 'key', key: 'space' });
    await w.out.wait(CAPSULE, (l) => l.startsWith('show'));
    const cap = await capsulePage(w.s);
    // the field that takes his keys: the app's composer (PmField testId 'spot.input' draws its
    // editable as 'spot.input.field', 33d67f61d), focused at show (Spotlight's selectAll)
    await expect(cap.getByTestId('spot.input.field'), 'the spotlight has the keys').toBeFocused({ timeout: 3000 });
    await sleep(300);
    const first = await frame(w);
    const seen: Frame[] = [first];
    // 20 keystrokes: matches, no match (the list empties), back to matches
    const back = ['Backspace', 'Backspace', 'Backspace', 'Backspace'];
    const keys = ['p', 'e', 'r', 'f', ...back, 's', 'h', 'o', 'p', ...back, 'z', 'q', 'x', 'w'];
    expect(keys).toHaveLength(20);
    for (const k of keys) {
      await cap.keyboard.press(k);
      await sleep(150); // the page sends its new size (capsule.size), main applies it
      seen.push(await frame(w));
    }
    await cap.screenshot({ path: test.info().outputPath('spot-after-20.png') });
    test.info().annotations.push({ type: 'sizes', description: sizes(seen).join(' ') });
    const moved = seen.filter((f) => f.x !== first.x || f.y !== first.y);
    expect(moved, `the frame's top-left stays at ${first.x},${first.y} through 20 keystrokes`).toEqual([]);
    expect(sizes(seen).length, 'the frame did resize as the list filtered (else this proves nothing)').toBeGreaterThan(1);
  });
});

test('native/capsule-pin (b): a voice session never moves the corner card\'s bottom-right, the glass stays above the Dock line', async () => {
  await onWorld(async (w) => {
    await w.helper.send({ ev: 'fn', act: 'talk_start', by: 'fn' });
    await w.out.wait(CAPSULE, (l) => l.startsWith('show corner'));
    const cap = await capsulePage(w.s);
    await sleep(300);
    const first = await frame(w);
    const seen: Frame[] = [first];
    const glasses: Frame[] = [await glass(w, cap, 'capsule.talk')];
    for (let i = 0; i < 10; i++) {
      await sleep(150);
      seen.push(await frame(w));
      glasses.push(await glass(w, cap, 'capsule.talk'));
    }
    await cap.screenshot({ path: test.info().outputPath('talk-listen.png') });
    await w.helper.send({ ev: 'fn', act: 'talk_end' }); // the release: bise thinks
    for (let i = 0; i < 10; i++) {
      await sleep(150);
      seen.push(await frame(w));
      const v = cap.getByTestId('capsule.talk');
      if (await v.isVisible()) glasses.push(await glass(w, cap, 'capsule.talk'));
    }
    await cap.screenshot({ path: test.info().outputPath('talk-think.png') });
    test.info().annotations.push({ type: 'sizes', description: sizes(seen).join(' ') });
    // the corner card is pinned at its bottom-right and grows upward (designer m_22118, amb-mac d6622e50)
    const corner = (g: Frame): string => `${Math.round(g.x + g.width)},${Math.round(g.y + g.height)}`;
    const g0 = glasses[0];
    if (!g0) throw new Error('no glass');
    const moved = glasses.filter((g) => Math.abs(g.x + g.width - (g0.x + g0.width)) > 1 || Math.abs(g.y + g.height - (g0.y + g0.height)) > 1).map(corner);
    expect(moved, `the glass's bottom-right stays at ${corner(g0)} through the session`).toEqual([]);
    expect(Math.abs(g0.y + g0.height - (DOCK_LINE - CORNER)), `the glass's bottom sits ${CORNER} px above the Dock line`).toBeLessThanOrEqual(2);
    const past = glasses.filter((g) => g.y + g.height > DOCK_LINE - CORNER + 2);
    expect(past, `the glass never passes ${CORNER} px above the Dock line`).toEqual([]);
  });
});
