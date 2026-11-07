// @vitest-environment happy-dom
// thread/steady-bottom, its right bar clause (main m_55267: the right bar holds still while the
// thread scrolls). A wheel scrolls the scroll box under the pointer, then chains to that box's
// ancestors only. The thread's scroller lives in the screen (app.screen); the right bar is the
// panel's column (.app-panelcol, PanelHost). They are two separate boxes of the frame, neither
// inside the other, so a wheel over the thread can never reach the right bar; above them the shell
// clips and the page never scrolls (shell-scroll.test.ts). The real App, mounted as the window
// mounts it. Mutation: App.tsx's frame with <Current /> inside .app-panelcol turns it red.
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { App } from './App';

afterEach(cleanup);

describe('thread/steady-bottom: the thread and the right bar are two scroll boxes', () => {
  it("the screen (the thread's box) and the panel's column are separate children of the frame", () => {
    const v = render(<App />);
    const frame = v.getByTestId('app.frame');
    const screen = v.getByTestId('app.screen');
    const bar = frame.querySelector('.app-panelcol');
    if (bar === null) throw new Error('no .app-panelcol in the frame');
    expect(bar.contains(screen), "the thread's box inside the right bar").toBe(false);
    expect(screen.contains(bar), "the right bar inside the thread's box").toBe(false);
    // each its own column of the frame's grid: the panel's column is the frame's own child
    expect(bar.parentElement).toBe(frame);
  });
});
