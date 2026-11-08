// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { artifactGesture, INPUT_EVERY_MS, wireUsageInput } from './usage';

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
