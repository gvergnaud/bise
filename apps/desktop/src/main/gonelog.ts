// A renderer or a child process that died or hung, as one app.log line (pr-crash, his m_68101: his
// 'the PR review crashes the renderer' left no trace at all: no line, no dump). Each window's
// renderer gone (its reason and exit code), a child process gone (the GPU, a utility), a page
// unresponsive and back; and Chromium's crash dumps kept locally (crashReporter, never uploaded),
// in the app's crashDumps folder (index.ts sets it under a stand-in home). Owner: pr-crash.
import { app, crashReporter, type Details, type RenderProcessGoneDetails, type WebContents } from 'electron';

/** What went: a window's renderer, another child process, a page that hangs or answers again. */
export type Gone =
  | { readonly kind: 'renderer'; readonly url: string; readonly details: Pick<RenderProcessGoneDetails, 'reason' | 'exitCode'> }
  | { readonly kind: 'child'; readonly details: Pick<Details, 'type' | 'reason' | 'exitCode' | 'name'> }
  | { readonly kind: 'unresponsive' | 'responsive'; readonly url: string };

/** Its app.log line (appLogLine stamps and cuts it); a clean exit says nothing. Pure. */
export function goneLine(g: Gone): string | null {
  switch (g.kind) {
    case 'renderer':
      if (g.details.reason === 'clean-exit') return null;
      return `renderer gone: ${g.details.reason} (exit ${String(g.details.exitCode)}) at ${pageOf(g.url)}`;
    case 'child':
      if (g.details.reason === 'clean-exit') return null;
      return `child gone: ${g.details.name ?? g.details.type} ${g.details.reason} (exit ${String(g.details.exitCode)})`;
    case 'unresponsive':
      return `renderer unresponsive at ${pageOf(g.url)}`;
    case 'responsive':
      return `renderer responsive again at ${pageOf(g.url)}`;
  }
}

/** a page's place without its query (a hub token never goes in a log) */
function pageOf(url: string): string {
  return url.split('?')[0] ?? '';
}

/** Starts the local dumps (call before ready) and writes each death or hang through `log`. */
export function installGoneLog(log: (line: string) => void): void {
  crashReporter.start({ uploadToServer: false });
  const say = (g: Gone): void => {
    const line = goneLine(g);
    if (line !== null) log(line);
  };
  app.on('render-process-gone', (_e, contents, details) => {
    say({ kind: 'renderer', url: contents.getURL(), details });
  });
  app.on('child-process-gone', (_e, details) => {
    say({ kind: 'child', details });
  });
  app.on('web-contents-created', (_e, contents: WebContents) => {
    contents.on('unresponsive', () => {
      say({ kind: 'unresponsive', url: contents.getURL() });
    });
    contents.on('responsive', () => {
      say({ kind: 'responsive', url: contents.getURL() });
    });
  });
}
