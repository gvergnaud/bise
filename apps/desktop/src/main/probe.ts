// Does an artifact's page server answer (page-unreach, Benjamin's blank panel)? The page server
// lives in its project's hub: a hub that is stopped (idle-exit), starting or gone leaves the
// panel's frame on Chromium's 'refused' page, which the window can't read (another origin). So
// before framing a pages-server url, and before '↗ browser' opens one, main asks it (a GET whose
// body it never reads). It answers 404: the artifact's file is gone ('gone', designer m_52746: the
// viewer draws its own 'file gone' state, never the server's raw line); it answers anything else:
// 'up'; nothing answers: 'down'. Only http://127.0.0.1:<port> is asked; any other url is not ours
// to judge: 'up'. Owner: page-unreach (the artifact view's reach, artifacts/reach.ts, reads it).
import { get } from 'node:http';
import { isPagesFrame } from '../shared/pagelink';

/** How long the server may take to answer before it counts as not answering. */
export const PROBE_MS = 800;

export type Reach = 'up' | 'down' | 'gone';

/** Pure: the port of a pages-server url, else null (not ours to probe). */
export function probePort(raw: unknown): number | null {
  if (typeof raw !== 'string' || !isPagesFrame(raw)) return null;
  const u = new URL(raw);
  const port = Number(u.port === '' ? '80' : u.port);
  return Number.isInteger(port) && port > 0 && port < 65536 ? port : null;
}

/** The url asked: 'up' when it answers (but 404) in `ms`, 'gone' on a 404, else 'down' (refused, timed out). */
export function httpProbe(url: string, ms = PROBE_MS): Promise<Reach> {
  return new Promise((resolve) => {
    let done = false;
    const end = (r: Reach) => {
      if (done) return;
      done = true;
      clearTimeout(t);
      req.destroy();
      resolve(r);
    };
    const req = get(url, (res) => {
      end(res.statusCode === 404 ? 'gone' : 'up');
      res.resume();
    });
    const t = setTimeout(() => {
      end('down');
    }, ms);
    req.once('error', () => {
      end('down');
    });
  });
}

/** The IPC's answer for `raw`: a pages-server url is asked, anything else is 'up'. */
export function probePage(raw: unknown, probe: (url: string) => Promise<Reach> = httpProbe): Promise<Reach> {
  return probePort(raw) === null || typeof raw !== 'string' ? Promise.resolve('up') : probe(raw);
}
