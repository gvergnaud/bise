// A take's tape (readme-film): every frame the compositor draws, written to disk as it comes (no
// decoding, so a 40 s take at 2x stays light), with its time, plus the story's marks (a beat, the
// box of what the cursor goes to) on the same clock. The film page (docs/brand/readme/film) reads
// tape.json and the frames.
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import type { CDPSession, ElectronApplication, Locator, Page } from '@playwright/test';

export type Box = { x: number; y: number; width: number; height: number };
export type Mark = { t: number; name: string; box?: Box; text?: string };
type Shot = { data: string; sessionId: number; metadata: { timestamp?: number } };

export class Tape {
  private readonly frames: { t: number; f: string }[] = [];
  private readonly marks: Mark[] = [];
  private n = 0;
  private t0 = 0;
  private on = false;

  constructor(private readonly cdp: CDPSession, private readonly page: Page, private readonly dir: string) {
    mkdirSync(path.join(dir, 'frames'), { recursive: true });
  }

  /**
   * The window drawn at `scale` device px per css px: an off-screen window paints at 1x whatever the
   * emulation says, so the window grows to w*scale × h*scale and its page zooms by `scale` (the
   * layout stays w × h css px, the boxes the marks log too).
   */
  async scale(app: ElectronApplication, width: number, height: number, scale: number): Promise<void> {
    await app.evaluate(({ BrowserWindow }, a) => {
      const w = BrowserWindow.getAllWindows().find((x) => !x.isDestroyed() && x.webContents.getURL().includes('index.html')) ?? BrowserWindow.getAllWindows()[0];
      if (w === undefined) throw new Error('no window');
      w.setMinimumSize(1, 1);
      w.setContentSize(a.width * a.scale, a.height * a.scale);
      w.webContents.setZoomFactor(a.scale);
    }, { width, height, scale });
    await this.page.waitForFunction(([w, h]) => window.innerWidth === w && window.innerHeight === h, [width, height]);
  }

  private readonly onFrame = (s: Shot): void => {
    void this.cdp.send('Page.screencastFrameAck', { sessionId: s.sessionId }).catch(() => undefined);
    if (!this.on) return;
    const t = (s.metadata.timestamp ?? Date.now() / 1000) - this.t0;
    const f = `${String(this.n++).padStart(5, '0')}.png`;
    writeFileSync(path.join(this.dir, 'frames', f), Buffer.from(s.data, 'base64'));
    this.frames.push({ t: Math.round(t * 1000) / 1000, f });
  };

  async start(): Promise<void> {
    this.t0 = Date.now() / 1000;
    this.on = true;
    this.cdp.on('Page.screencastFrame', this.onFrame);
    await this.cdp.send('Page.startScreencast', { format: 'png', everyNthFrame: 1 });
  }

  now(): number {
    return Math.round((Date.now() / 1000 - this.t0) * 1000) / 1000;
  }

  async mark(name: string, at?: Locator, text?: string): Promise<void> {
    const b = at === undefined ? null : await at.boundingBox({ timeout: 3000 }).catch(() => null);
    this.marks.push({ t: this.now(), name, ...(b === null ? {} : { box: b }), ...(text === undefined ? {} : { text }) });
  }

  async stop(meta: Record<string, unknown>): Promise<void> {
    await this.cdp.send('Page.stopScreencast').catch(() => undefined);
    this.on = false;
    this.cdp.off('Page.screencastFrame', this.onFrame);
    writeFileSync(path.join(this.dir, 'tape.json'), JSON.stringify({ ...meta, end: this.now(), frames: this.frames, marks: this.marks }, null, 1));
  }
}
