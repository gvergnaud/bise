// src/views · the host context a view gets at ui/initialize and in host-context-changed (spec
// HostContext; docs/mcp-apps.md C7): its theme, our design tokens mapped on the spec's CSS
// variables (their computed values: a view can't read our var() names; the line heights read out of
// our --f-* type styles; the ground --c-card, a thread card's fill made opaque), its display mode and
// size, his locale and time zone, the tool it shows, a document view's document and kept state. Pure but for readTokens, which reads the
// page's computed styles once per theme. Owner: mcp-apps-host.
import type { DisplayMode, Json, JsonObject } from './protocol';
import { DISPLAY_MODES } from './protocol';

/**
 * The spec's style variables, each from one of our tokens (src/ds/tokens.css). A spec name not
 * listed here is left to the view's own fallback.
 */
export const STYLE_MAP: Readonly<Record<string, string>> = {
  // the view's ground: a thread card's fill, opaque (designer m_51798)
  '--color-background-primary': '--c-card',
  '--color-background-secondary': '--c-sunken',
  '--color-background-tertiary': '--c-hover',
  '--color-background-inverse': '--c-ink',
  '--color-background-info': '--c-accent-soft',
  '--color-background-danger': '--c-err-soft',
  '--color-background-success': '--c-ok-soft',
  '--color-text-primary': '--c-text',
  '--color-text-secondary': '--c-text-2',
  '--color-text-tertiary': '--c-text-3',
  '--color-text-inverse': '--c-on-ink',
  '--color-text-info': '--c-accent',
  '--color-text-danger': '--c-err',
  '--color-text-success': '--c-ok',
  '--color-border-primary': '--c-line-strong',
  '--color-border-secondary': '--c-line',
  '--color-border-info': '--c-accent-line',
  '--color-ring-primary': '--c-focus',
  '--font-sans': '--font-ui',
  '--font-mono': '--font-mono',
  // the spec's type sizes on bise's scale: its body (md) is bise's 13 (designer m_50450)
  '--font-text-xs-size': '--t-11',
  '--font-text-sm-size': '--t-12',
  '--font-text-md-size': '--t-13',
  '--font-text-lg-size': '--t-15',
  '--font-heading-xs-size': '--t-13',
  '--font-heading-sm-size': '--t-15',
  '--font-heading-md-size': '--t-17',
  '--font-heading-lg-size': '--t-22',
  '--font-heading-xl-size': '--t-30',
  '--border-radius-sm': '--r-1',
  '--border-radius-md': '--r-2',
  '--border-radius-lg': '--r-3',
  '--border-radius-xl': '--r-4',
  '--border-radius-full': '--r-pill',
  '--shadow-sm': '--e-1',
  '--shadow-md': '--e-2',
  '--shadow-lg': '--e-3',
};

/**
 * The spec's line heights, each the line height of one of our type styles (tokens.css's --f-*,
 * a `font` shorthand `<size>/<line-height> <family>`): meta 12/16, ui 13/18, lead 15/22.
 */
export const LINE_HEIGHT_MAP: Readonly<Record<string, string>> = {
  '--font-text-xs-line-height': '--f-kbd',
  '--font-text-sm-line-height': '--f-meta',
  '--font-text-md-line-height': '--f-ui',
  '--font-text-lg-line-height': '--f-lead',
  '--font-heading-xs-line-height': '--f-ui',
  '--font-heading-sm-line-height': '--f-lead',
  '--font-heading-lg-line-height': '--f-title',
  '--font-heading-xl-line-height': '--f-question',
};

/** The line height of a `font` shorthand's value (`.75rem/16px -apple-system`), or ''. Pure. */
export function lineHeightOf(font: string): string {
  return /\/\s*([\d.]+(?:px|rem|em)?)\b/.exec(font)?.[1] ?? '';
}

/** The spec's variables with their values, from a reader of our tokens' computed values; an empty one left out. Pure. */
export function styleVariables(read: (token: string) => string): JsonObject {
  const out: JsonObject = {};
  for (const [spec, ours] of Object.entries(STYLE_MAP)) {
    const v = read(ours).trim();
    if (v !== '') out[spec] = v;
  }
  for (const [spec, ours] of Object.entries(LINE_HEIGHT_MAP)) {
    const v = lineHeightOf(read(ours));
    if (v !== '') out[spec] = v;
  }
  return out;
}

/** Our tokens' computed values on the page's root (what styleVariables reads). */
export function readTokens(root: Element = document.documentElement): (token: string) => string {
  const cs = getComputedStyle(root);
  return (t) => cs.getPropertyValue(t);
}

/** The theme in use: his pref's attribute on <html>, else the system's. */
export function themeNow(root: HTMLElement = document.documentElement): 'light' | 'dark' {
  const t = root.dataset['theme'];
  if (t === 'light' || t === 'dark') return t;
  return typeof matchMedia === 'function' && matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

/** Where a document's view gets its state back (the RFC's ai.mistral.vibe/viewState, A5). */
export const VIEW_STATE_KEY = 'ai.mistral.vibe/viewState';
/** Where a document's view reads what changed since he last looked (A5). */
export const CHANGED_KEY = 'dev.bise/changed';

/** What a context is made of. */
export interface ContextIn {
  readonly theme: 'light' | 'dark';
  readonly variables: JsonObject;
  readonly mode: DisplayMode;
  /** the frame's size now (css px); the inline card lets the view choose its height */
  readonly width: number;
  readonly height: number;
  readonly locale: string;
  readonly timeZone: string;
  readonly tool: JsonObject | null;
  /** a document's view (docs/plugin-apps.md A5): the document, and its state kept by the host */
  readonly document?: JsonObject;
  readonly viewState?: Json;
  /** what changed since he last looked (`dev.bise/changed`, A5) */
  readonly changed?: JsonObject;
}

/** The spec's HostContext. Pure. */
export function hostContext(c: ContextIn): JsonObject {
  return {
    theme: c.theme,
    styles: { variables: c.variables },
    displayMode: c.mode,
    availableDisplayModes: [...DISPLAY_MODES],
    containerDimensions: c.mode === 'inline' ? { width: c.width, maxHeight: c.height } : { width: c.width, height: c.height },
    locale: c.locale,
    timeZone: c.timeZone,
    platform: 'desktop',
    ...(c.tool === null ? {} : { toolInfo: { tool: c.tool } }),
    ...(c.document === undefined ? {} : { document: c.document }),
    ...(c.viewState === undefined ? {} : { [VIEW_STATE_KEY]: c.viewState }),
    ...(c.changed === undefined ? {} : { [CHANGED_KEY]: c.changed }),
  };
}

/**
 * What changed between two contexts, for ui/notifications/host-context-changed: the top-level
 * keys whose value differs (deep), or null when none did. Pure.
 */
export function contextChange(before: JsonObject, after: JsonObject): JsonObject | null {
  const out: JsonObject = {};
  for (const [k, v] of Object.entries(after)) if (JSON.stringify(before[k]) !== JSON.stringify(v)) out[k] = v;
  return Object.keys(out).length === 0 ? null : out;
}
