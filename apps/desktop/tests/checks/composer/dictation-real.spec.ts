// composer/dictation, real core · bar 2.6 + I (docs/desktop-bar.md) · his bug 3's voice.
// The new app on the REAL core of a throwaway world (tests/checks/real.sh brings it up and takes it
// down; world.ts), in bise's own thread with its real composer. The core's fake voice
// (BISE_AMBIENT_FAKE_VOICE, rust/tui/src/ambient/fake.rs: the talk's ports, no mic, no speech to
// text, no sound) hears the words of a file, read and removed at each dictation's start, one word
// every 150 ms. Real keys only. Passes when, with ctrl+r held, the words go into the field after
// what he typed, the mic shows recording then off, the meter moves, the field never flashes, and
// nothing reaches main until he sends (main's transcript, read outside the app); then ⏎ with the
// mic on stops it without sending, and the next ⏎ sends the words to main.
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import { test, expect, Recorder, flashes, underLoad } from '../../harness';
import { launchReal, realWorld, transcriptHas } from '../world';
import { fieldText } from './target';

const S = {
  field: '[data-testid="composer.field"]',
  mic: '[data-testid="composer.mic"]',
  /** the take's chip's bars in the field (designer m_22700), data-bars the 6 steps */
  meter: '[data-testid="composer.dictation.meter"]',
  setupSkip: '[data-testid="setup.skip"]',
};
const HELD = 'bonjour à tous, on garde la file';
const TAPPED = 'et on livre vendredi';

test('composer/dictation (real core): ctrl+r held in the real thread, the core’s words land, nothing reaches main until sent', async () => {
  // ~19 s on an idle Mac: its 60 s stretched by the load (harness/load.ts: the real core's launch
  // and the fake voice's words ran past 60 s at load ~15 in 11.11's pass, main m_64400)
  test.setTimeout(underLoad(60_000));
  const w = realWorld();
  const voice = path.join(w.home, 'tmp', 'qa-type-voice.txt');
  const s = await launchReal(w, { env: { BISE_AMBIENT_FAKE_VOICE: voice } });
  try {
    const { page, cdp } = s;
    const field = page.locator(S.field);
    // a fresh home opens setup once (amb-home c44c926d): skip it the way he would, by a click
    await expect(page.locator(`${S.field}, ${S.setupSkip}`).first()).toBeVisible({ timeout: 15_000 }).catch(() => undefined);
    if (await page.locator(S.setupSkip).isVisible()) await page.locator(S.setupSkip).click();
    await expect(field, "bise's thread opens with its composer on the real core").toBeVisible({ timeout: 15_000 });
    const text = async (): Promise<string> => (await field.evaluate(fieldText)).trim();
    const mic = (): Promise<string | null> => page.locator(S.mic).getAttribute('data-state');
    await field.click();
    await page.keyboard.type('note: ');
    const box = await field.boundingBox();
    if (!box) throw new Error('no field box');

    writeFileSync(voice, HELD);
    const rec = new Recorder(cdp, page);
    await rec.start();
    await page.keyboard.down('Control');
    await page.keyboard.down('r');
    await expect.poll(mic, { message: 'held: the mic is on (the core opened it)' }).toBe('recording');
    const levels = new Set<string>();
    for (let i = 0; i < 12; i++) {
      levels.add((await page.locator(S.meter).getAttribute('data-bars').catch(() => null)) ?? '');
      await page.waitForTimeout(100);
    }
    await page.keyboard.up('r');
    await page.keyboard.up('Control');
    await expect.poll(text, { timeout: 8000, message: "the core's words follow what he typed" }).toBe(`note: ${HELD}`);
    await expect.poll(mic, { message: 'released: the mic is off' }).toBe('off');
    const frames = await rec.stop();
    levels.delete('');
    expect.soft(levels.size, `the meter moved with the fake voice (${[...levels].join(', ')})`).toBeGreaterThan(1);
    expect.soft(flashes(frames, box), 'frames where the field flashed while the words landed').toEqual([]);
    await page.waitForTimeout(1500);
    expect(transcriptHas(w, 'bise', 'main', 'bonjour à tous'), 'nothing reached main: he has not sent').toBe(false);

    // a tap, then ⏎: the mic stops, nothing is sent; the next ⏎ sends all of it to main
    await page.keyboard.type(' ');
    writeFileSync(voice, TAPPED);
    await page.keyboard.press('Control+r');
    await expect.poll(mic).toBe('recording');
    await page.waitForTimeout(900);
    await page.keyboard.press('Enter');
    await expect.poll(mic, { message: '⏎ stops the mic' }).toBe('off');
    await expect.poll(text, { timeout: 8000 }).toBe(`note: ${HELD} ${TAPPED}`);
    await page.waitForTimeout(1500);
    expect(transcriptHas(w, 'bise', 'main', TAPPED), '⏎ while dictating sent nothing').toBe(false);
    await page.keyboard.press('Enter');
    await expect.poll(() => transcriptHas(w, 'bise', 'main', `note: ${HELD} ${TAPPED}`), { timeout: 15_000, message: 'the next ⏎ sends the words to main' }).toBe(true);
  } finally {
    await s.close();
  }
});
