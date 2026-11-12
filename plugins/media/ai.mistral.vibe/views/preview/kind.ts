// Preview · which of its renderers draws a file, and how its bytes reach it. The hub picked
// Preview by the file's name (view.json's dev.bise/ui.files, bend_plugins::opens); this reads the
// same name for which of the three it is.
//
// Pure (tested by plugins/media/test/kind.test.mjs).

export type Kind = "image" | "pdf" | "video";

const KINDS: Record<string, Kind> = {
  png: "image", jpg: "image", jpeg: "image", gif: "image", webp: "image",
  pdf: "pdf",
  mp4: "video", mov: "video", webm: "video", m4v: "video",
};

/** The kind of a file by its name's ending (any case), or null: not one Preview opens. */
export function kindOf(name: string): Kind | null {
  const dot = name.lastIndexOf(".");
  if (dot < 0) return null;
  return KINDS[name.slice(dot + 1).toLowerCase()] ?? null;
}

/** The most one read gives (@bise/view's READ_MAX, docs/plugin-apps.md A4). */
export const READ_MAX = 24 * 1024 * 1024;

/** How the renderer gets the file: a video always by its session url (played by its ranges,
 * never read whole); a pdf by its url when it's bigger than one read (pdf.js reads its ranges);
 * else its bytes, in one read. */
export function wayOf(kind: Kind, size: number): "url" | "bytes" {
  if (kind === "video") return "url";
  return kind === "pdf" && size > READ_MAX ? "url" : "bytes";
}

/** His words for what went wrong: what happened, never a stack. */
export function words(e: { code?: unknown; message?: unknown } | null | undefined, name: string): string {
  const code = e && e.code;
  if (code === -32041 || code === -32039) return `${name} is gone: it was moved or deleted.`;
  if (code === "unreadable") return `${name} can't be shown: ${String(e!.message)}.`;
  const m = e && e.message ? String(e.message) : String(e);
  return `${name} could not be opened: ${m}.`;
}
