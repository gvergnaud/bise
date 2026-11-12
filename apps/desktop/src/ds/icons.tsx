// src/ds · the kind icons (artifacts-gallery v7, the designer agent's font audit): 16 px line icons,
// stroke 1.5, in currentColor (the screens set text-3), one per kind an artifact can be: page, doc,
// sheet, deck, image, video, audio, 3D, file, link, plus a PR and a release (their tiles). No
// Unicode kind glyph anywhere (▦ ▭ ♪ ◇ are missing from the fonts or of ambiguous width). One play
// triangle (PlayIcon), the same on every video. The kind is never in words on a card: the icon, or
// the extension on hover. A file's icon is its app's (the row's opener), never its ending's.
// iconKindOf and extOf are pure; the icons are a pure render. Owner: art-folders.

/** What an icon can draw. */
export type IconKind = "page" | "doc" | "sheet" | "deck" | "image" | "video" | "audio" | "3d" | "file" | "link" | "pr" | "release";

/** What iconKindOf reads of an artifact: its app (the hub's, A3), its kind word and where it is. */
export interface KindSource {
  readonly kind: string;
  readonly path?: string | null;
  readonly target?: string | null;
  readonly url?: string | null;
  /** the app that opens it, as the hub picked it (bend_plugins::opens): its icon wins */
  readonly opener?: { readonly icon?: string | null; readonly builtin?: boolean } | null;
}

// the words an artifact's kind says when it is no file (an agent's `--kind`, a PR, a link)
const WORD: Readonly<Record<string, IconKind>> = {
  page: "page", site: "page",
  link: "link", url: "link",
  pr: "pr", release: "release",
  image: "image", shot: "image", shots: "image",
  video: "video", audio: "audio", sound: "audio",
  "3d": "3d", model: "3d",
  doc: "doc", sheet: "sheet", deck: "deck", slides: "deck",
};

const isIconKind = (s: string | null | undefined): s is IconKind => s != null && Object.hasOwn(LINES, s);

/** The extension of what it points at, lowercase, '' none: 'png', 'xlsx' (the card's hover corner). Pure. */
export function extOf(a: KindSource): string {
  for (const where of [a.path, a.target, a.url]) {
    if (where == null || where === "") continue;
    if (/^(https?:|mailto:)/iu.test(where)) {
      const file = /\/[^/?#]+\.([a-z0-9]{1,8})(?:[?#]|$)/iu.exec(where.replace(/^[a-z]+:\/\/[^/]+/iu, ""));
      if (file?.[1] !== undefined) return file[1].toLowerCase();
      continue;
    }
    const m = /\.([a-z0-9]{1,8})$/iu.exec(where.replace(/\/+$/u, ""));
    if (m?.[1] !== undefined) return m[1].toLowerCase();
  }
  return "";
}

/**
 * The icon of an artifact: its app's (docs/plugin-apps.md A6b: a bise type's ds icon from
 * bend_plugins::opens' table; a plugin's own app draws as a file until its icon route serves),
 * else its kind word (a PR, a page, a link...), else a link for a url, else a file. Nothing here
 * reads a file's ending: the hub matched it once. Pure.
 */
export function iconKindOf(a: KindSource): IconKind {
  const o = a.opener;
  if (o != null) return o.builtin === true && isIconKind(o.icon) ? o.icon : "file";
  const word = WORD[a.kind.toLowerCase()];
  if (word !== undefined) return word;
  return /^https?:\/\//iu.test(a.url ?? "") ? "link" : "file";
}

/** The lines of each icon on a 16 px grid. */
const LINES: Readonly<Record<IconKind, readonly string[]>> = {
  page: ["M3.5 2.75h9a1.25 1.25 0 0 1 1.25 1.25v8a1.25 1.25 0 0 1-1.25 1.25h-9A1.25 1.25 0 0 1 2.25 12V4A1.25 1.25 0 0 1 3.5 2.75z", "M2.25 6h11.5"],
  doc: ["M4 1.75h5l3 3v9.5H4z", "M9 1.75v3h3", "M6 8.5h4M6 11h4"],
  sheet: ["M3.75 2.25h8.5a1.5 1.5 0 0 1 1.5 1.5v8.5a1.5 1.5 0 0 1-1.5 1.5h-8.5a1.5 1.5 0 0 1-1.5-1.5v-8.5a1.5 1.5 0 0 1 1.5-1.5z", "M2.25 6.25h11.5M6.25 6.25v7.5"],
  deck: ["M3 2.75h10a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1v-6.5a1 1 0 0 1 1-1z", "M8 11.25v2.5M5.5 13.75h5"],
  image: ["M3.5 2.75h9a1.25 1.25 0 0 1 1.25 1.25v8a1.25 1.25 0 0 1-1.25 1.25h-9A1.25 1.25 0 0 1 2.25 12V4A1.25 1.25 0 0 1 3.5 2.75z", "M2.5 11.5l3.5-3.5 2.5 2.5 2-2 3 3", "M6 5.25a1 1 0 1 1 0 2 1 1 0 0 1 0-2z"],
  video: ["M3.5 3.25h9a1.25 1.25 0 0 1 1.25 1.25v7a1.25 1.25 0 0 1-1.25 1.25h-9A1.25 1.25 0 0 1 2.25 11.5v-7A1.25 1.25 0 0 1 3.5 3.25z", "M6.75 6v4l3.25-2z"],
  audio: ["M3 7v2M5.5 5v6M8 2.5v11M10.5 5.5v5M13 7v2"],
  "3d": ["M8 1.75l5.5 3v6.5L8 14.25l-5.5-3v-6.5z", "M2.5 4.75L8 7.75l5.5-3M8 7.75v6.5"],
  file: ["M4 1.75h5l3 3v9.5H4z", "M9 1.75v3h3"],
  link: ["M6.75 9.25a2.5 2.5 0 0 0 3.5 0l2-2a2.5 2.5 0 0 0-3.5-3.5l-.75.75", "M9.25 6.75a2.5 2.5 0 0 0-3.5 0l-2 2a2.5 2.5 0 0 0 3.5 3.5l.75-.75"],
  pr: ["M4.5 2.5a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zM4.5 10.5a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zM11.5 10.5a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3z", "M4.5 5.5v5M11.5 10.5V7a2 2 0 0 0-2-2H7.25", "M8.75 3.5 7.25 5l1.5 1.5"],
  release: ["M2.25 8V2.25H8l6 6-5.75 5.75z", "M5.25 4.25a1 1 0 1 1 0 2 1 1 0 0 1 0-2z"],
};

/** A kind's path data on its 16 grid, for a DOM drawn without React (the composer's chips). Pure. */
export function kindIconPaths(kind: IconKind): readonly string[] {
  return LINES[kind];
}

/** A kind's icon: 16 px (or `size`), stroke 1.5, currentColor; decorative (its words are the card's tip). */
export function KindIcon({ kind, size = 16, testId }: { kind: IconKind; size?: number; testId?: string }) {
  return (
    <svg className="b-kind-icon" viewBox="0 0 16 16" width={size} height={size} aria-hidden="true" data-kind={kind} data-testid={testId}
      fill="none" stroke="currentColor" strokeWidth={1.5} strokeLinecap="round" strokeLinejoin="round">
      {LINES[kind].map((d) => <path key={d} d={d} />)}
    </svg>
  );
}

/** The one play triangle (a video's mark, '▶ 0:04'), filled in currentColor. */
export function PlayIcon({ size = 10 }: { size?: number }) {
  return (
    <svg className="b-play-icon" viewBox="0 0 10 10" width={size} height={size} aria-hidden="true" fill="currentColor">
      <path d="M2 1.5v7l6-3.5z" />
    </svg>
  );
}
