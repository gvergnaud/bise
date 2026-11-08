// @vitest-environment happy-dom
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ARTIFACT_OTHER_KINDS, artifactGesture, INPUT_EVERY_MS, wireUsageInput } from './usage';

afterEach(() => {
  vi.useRealTimers();
});

describe("the window's usage signals", () => {
  it('sends his input as a bare signal, at most once every 5 s', () => {
    vi.useFakeTimers();
    const input = vi.fn();
    (window as unknown as { bise: unknown }).bise = { usage: { input, gesture: vi.fn() } };
    wireUsageInput();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'c' }));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'a' }));
    expect(input).toHaveBeenCalledTimes(1);
    // no content: not the key, not the event
    expect(input.mock.calls[0]).toEqual([]);
    vi.advanceTimersByTime(INPUT_EVERY_MS);
    window.dispatchEvent(new PointerEvent('pointerdown'));
    expect(input).toHaveBeenCalledTimes(2);
  });

  it("knows every artifact kind the hub makes: its own gesture, or other by name", () => {
    const src = readFileSync(join(__dirname, '../../../../rust/switchboard/src/artifacts.rs'), 'utf8');
    const body = (fn: string): string => {
      const start = src.indexOf(`pub fn ${fn}(`);
      return src.slice(start, src.indexOf('\n}\n', start));
    };
    // what each fn returns: `return "x"`, `=> "x"`, `{ "x" }` and its last line's "x"
    const made = new Set([...['link_kind', 'path_kind'].flatMap((fn) => [...body(fn).matchAll(/(?:return|=>|\{)[ \t]*"([a-z]+)"|\n\s*"([a-z]+)"\s*$/g)].map((m) => m[1] ?? m[2]))]);
    expect(made.size).toBeGreaterThan(8);
    const own = [...made].filter((k) => artifactGesture(k) === `artifact_${k}`);
    expect([...own, ...ARTIFACT_OTHER_KINDS].sort()).toEqual([...made].sort());
  });

  it("names an artifact's gesture by its kind, any other kind as other", () => {
    expect(['page', 'doc', 'sheet', 'slides', 'image', 'link', 'code', 'pr', 'video', '/Users/camille/x.md'].map(artifactGesture)).toEqual([
      'artifact_page',
      'artifact_doc',
      'artifact_sheet',
      'artifact_slides',
      'artifact_image',
      'artifact_link',
      'artifact_code',
      'artifact_other',
      'artifact_other',
      'artifact_other',
    ]);
  });
});
