import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it, vi } from 'vitest';
import { appendCapsuleFrame, capsuleFrame, capsuleFramesDir, CAPSULE_FRAMES_FILE } from './capsuleframe';
import { capsuleBounds, CORNER, dockOf, OFF, type DisplayBox } from './place';

// a 1512x982 MacBook display: the menu bar 33 px, the Dock 70 px at the bottom
const mac: DisplayBox = {
  bounds: { x: 0, y: 0, width: 1512, height: 982 },
  workArea: { x: 0, y: 33, width: 1512, height: 879 },
};
const parked = { x: OFF, y: OFF, width: 420, height: 160 };

// ambient-lead m_22598, amb-tools m_22601: the corner card's frame readable in an off-screen packed run
describe('capsule frames under the off-screen knob', () => {
  it('recorded only off-screen, never in test mode nor in his normal launch', () => {
    expect(capsuleFramesDir(true, false, '/ud')).toBe('/ud');
    expect(capsuleFramesDir(false, false, '/ud')).toBeNull();
    expect(capsuleFramesDir(true, true, '/ud')).toBeNull();
    expect(capsuleFramesDir(false, true, '/ud')).toBeNull();
  });

  it("a corner show: where place.ts puts it on his display, that display's work area, screen and Dock, and the parked frame", () => {
    const b = capsuleBounds({ width: 420, height: 160 }, mac, { at: 'corner' });
    const f = capsuleFrame(1000, 'show', 'corner', b, mac, parked);
    expect(f).toEqual({ t: 1000, ev: 'show', mode: 'corner', at: 'corner', bounds: b, work: mac.workArea, screen: mac.bounds, dock: 'bottom', parked });
    // its bottom-right 16 px inside the work area's
    expect(f.bounds.x + f.bounds.width).toBe(1512 - CORNER);
    expect(f.bounds.y + f.bounds.height).toBe(33 + 879 - CORNER);
    expect(capsuleFrame(1, 'move', 'bottom', b, mac, parked).mode).toBe('center');
  });

  it('the Dock side: bottom, right, left, or hidden (a gap under 8 px)', () => {
    const at = (workArea: DisplayBox['workArea']): string => dockOf({ bounds: mac.bounds, workArea });
    expect(at(mac.workArea)).toBe('bottom');
    expect(at({ x: 0, y: 33, width: 1440, height: 949 })).toBe('right');
    expect(at({ x: 72, y: 33, width: 1440, height: 949 })).toBe('left');
    expect(at({ x: 0, y: 33, width: 1512, height: 945 })).toBe('hidden');
  });

  it('each record is one JSON line appended in order to userData/capsule-frames.jsonl', () => {
    const dir = mkdtempSync(join(tmpdir(), 'capsule-frames-'));
    try {
      const b = capsuleBounds({ width: 420, height: 160 }, mac, { at: 'corner' });
      appendCapsuleFrame(dir, capsuleFrame(1, 'show', 'corner', b, mac, parked));
      appendCapsuleFrame(dir, capsuleFrame(2, 'move', 'corner', { ...b, y: b.y - 40, height: 200 }, mac, parked));
      const lines = readFileSync(join(dir, CAPSULE_FRAMES_FILE), 'utf8').trim().split('\n');
      expect(lines.map((l) => (JSON.parse(l) as { ev: string }).ev)).toEqual(['show', 'move']);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("a write that fails throws nothing (the app goes on) and says it on stderr", () => {
    // caught here, not printed: the gate showed this expected line first under
    // 'FAIL desktop_check' in 3 of 8 reds (loop audit 11-03), never their cause
    const said: string[] = [];
    const err = vi.spyOn(process.stderr, 'write').mockImplementation((s: string | Uint8Array) => {
      said.push(String(s));
      return true;
    });
    try {
      expect(() => {
        appendCapsuleFrame('/nonexistent/dir/for/sure', capsuleFrame(1, 'show', 'corner', parked, mac, parked));
      }).not.toThrow();
    } finally {
      err.mockRestore();
    }
    expect(said.join('')).toContain('bise-desktop: capsule-frames: Error: ENOENT');
  });
});
