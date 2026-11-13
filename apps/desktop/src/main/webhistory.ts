// The web pages he opened from bise, for ⌘T's omnibox (page newtab-omnibox-flow; architect
// m_64583). bise loads no web page in-app: every http(s) link leaves through openLink (ipc.ts), and
// that door notes a visit here. Pure: what is never recorded (a URL with user:pass@, loopback and
// local hosts, which carry bise's own tokens, every scheme but http and https), a visit noted (the
// page to the front, its last VISITS_KEPT visits, `typed` when he typed it in the bar), the prune
// (visits older than KEEP_DAYS, MAX_PAGES pages), a page removed, all cleared, his switch, and the
// file's words read back (a broken file: empty, said). The shell is webhistoryipc.ts, the one
// writer. Never in a hub: no agent, sb or telemetry sees it. Owner: newtab-omnibox.
import { join } from 'node:path';
import type { WebHistory, WebPage, WebVisit } from '../shared/api';

/** How many pages it keeps at most (the least recent go first). */
export const MAX_PAGES = 2000;
/** A visit older than this many days goes (Chrome's 90). */
export const KEEP_DAYS = 90;
/** How many visits a page keeps (frecency reads the last 10, as Chrome's). */
export const VISITS_KEPT = 10;
/** The longest URL it keeps. */
export const URL_MAX = 2048;

const DAY_MS = 86_400_000;

/** Its file in `dir`, the app's userData (a test copy's own: never his). Pure. */
export function webHistoryFile(dir: string): string {
  return join(dir, 'web-history.json');
}

/** Nothing yet: his switch on (designer m_64584: on by default, local only). */
export const EMPTY_HISTORY: WebHistory = { keep: true, pages: [] };

/** A loopback or local host: bise's own pages and view origins (tokens in their URLs). Pure. */
function localHost(host: string): boolean {
  const h = host.toLowerCase().replace(/^\[|\]$/gu, '');
  return h === 'localhost' || h.endsWith('.localhost') || h === '::1' || h === '0.0.0.0' || /^127\.\d+\.\d+\.\d+$/u.test(h);
}

/** The URL as kept, or null when it is never recorded (not http(s), user:pass@, a local host, too long). Pure. */
export function recordable(raw: unknown): string | null {
  if (typeof raw !== 'string' || raw.length > URL_MAX) return null;
  let u: URL;
  try {
    u = new URL(raw);
  } catch {
    return null;
  }
  if (u.protocol !== 'http:' && u.protocol !== 'https:') return null;
  if (u.username !== '' || u.password !== '' || u.hostname === '' || localHost(u.hostname)) return null;
  const s = u.toString();
  return s.length > URL_MAX ? null : s;
}

/** `url` opened at `at` (`typed`: he typed it in the bar): its page first, its visit first; nothing while his switch is off. Pure. */
export function noteVisit(h: WebHistory, url: string, at: number, typed: boolean): WebHistory {
  if (!h.keep) return h;
  const old = h.pages.find((p) => p.url === url);
  const visit: WebVisit = { at, typed };
  const page: WebPage = { url, n: (old?.n ?? 0) + 1, visits: [visit, ...(old?.visits ?? [])].slice(0, VISITS_KEPT) };
  return { keep: true, pages: [page, ...h.pages.filter((p) => p.url !== url)].slice(0, MAX_PAGES) };
}

/** Visits older than KEEP_DAYS gone, a page with none left gone, newest page first. Pure. */
export function pruned(h: WebHistory, now: number): WebHistory {
  const since = now - KEEP_DAYS * DAY_MS;
  const pages = h.pages
    .map((p) => ({ ...p, visits: p.visits.filter((v) => v.at >= since) }))
    .filter((p) => p.visits.length > 0)
    .toSorted((a, b) => (b.visits[0]?.at ?? 0) - (a.visits[0]?.at ?? 0))
    .slice(0, MAX_PAGES);
  return { keep: h.keep, pages };
}

/** Without that page (his ⇧⌫ or the row's ×). Pure. */
export function withoutUrl(h: WebHistory, url: string): WebHistory {
  return { keep: h.keep, pages: h.pages.filter((p) => p.url !== url) };
}

/** Every page gone, his switch kept. Pure. */
export function cleared(h: WebHistory): WebHistory {
  return { keep: h.keep, pages: [] };
}

/** His switch 'Keep the links I open': off clears it and records nothing. Pure. */
export function withKeep(h: WebHistory, on: boolean): WebHistory {
  return { keep: on, pages: on ? h.pages : [] };
}

function visitOf(raw: unknown): WebVisit | null {
  if (raw === null || typeof raw !== 'object') return null;
  const r = raw as Record<string, unknown>;
  return typeof r['at'] === 'number' && Number.isFinite(r['at']) ? { at: r['at'], typed: r['typed'] === true } : null;
}

function pageOf(raw: unknown): WebPage | null {
  if (raw === null || typeof raw !== 'object') return null;
  const r = raw as Record<string, unknown>;
  const url = recordable(r['url']);
  const visits = (Array.isArray(r['visits']) ? (r['visits'] as unknown[]) : []).map(visitOf).filter((v): v is WebVisit => v !== null).slice(0, VISITS_KEPT);
  if (url === null || visits.length === 0) return null;
  const n = typeof r['n'] === 'number' && r['n'] >= visits.length ? Math.floor(r['n']) : visits.length;
  return { url, n, visits };
}

/** The file's words: its history, `broken` when they were there but unreadable (moved aside by the shell). Pure. */
export function parseHistory(text: string | null): { readonly h: WebHistory; readonly broken: boolean } {
  if (text === null) return { h: EMPTY_HISTORY, broken: false };
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return { h: EMPTY_HISTORY, broken: true };
  }
  if (raw === null || typeof raw !== 'object' || (raw as Record<string, unknown>)['v'] !== 1) return { h: EMPTY_HISTORY, broken: true };
  const r = raw as Record<string, unknown>;
  const pages = (Array.isArray(r['pages']) ? (r['pages'] as unknown[]) : []).map(pageOf).filter((p): p is WebPage => p !== null);
  const keep = r['keep'] !== false;
  return { h: { keep, pages: keep ? pages : [] }, broken: false };
}

/** Its words on disk, versioned. Pure. */
export function historyText(h: WebHistory): string {
  return JSON.stringify({ v: 1, keep: h.keep, pages: h.pages });
}
