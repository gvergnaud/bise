// A note's anchor as the SDK takes it (bise_proto::anchor, generated into
// ../anchor.gen.ts). `readAnchor` checks one that came from outside (a
// note's JSON): a kind this version does not know is refused with words,
// never guessed (architect m_23712).

import { ANCHOR_KINDS, type Anchor, type Rect } from "../anchor.gen.ts";
import { OfficeError } from "./errors.ts";
import type { FileKind } from "./opc.ts";

export { ANCHOR_KINDS };
export type { Anchor, Rect };
export type AnchorKind = (typeof ANCHOR_KINDS)[number];

// the generated list and the generated union are the same set
type Same<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;
const _kindsMatch: Same<AnchorKind, Anchor["kind"]> = true;
void _kindsMatch;

/** The anchor kinds that point into each file type. */
export const KINDS_OF: Record<FileKind, readonly AnchorKind[]> = {
  xlsx: ["cell"],
  pptx: ["slide", "shape"],
  docx: ["para"],
};

const FIELDS: Record<AnchorKind, Record<string, "string" | "count" | "rect" | "seconds" | "string?" | "count?" | "rect?">> = {
  block: { block: "string", quote: "string?" },
  cell: { sheet: "string", range: "string" },
  slide: { slide: "count", rect: "rect?" },
  shape: { slide: "count", shape: "string", quote: "string?" },
  para: { para: "string", quote: "string?" },
  region: { page: "count?", rect: "rect" },
  text: { line: "count", quote: "string?" },
  // a video's moment, or a box on its frame there (video-view): t in seconds, finite, 0 or more
  time: { t: "seconds", rect: "rect?" },
  // a plugin's view's own place (plugin-apps A5): its opaque key, and what he selected
  view: { key: "string", quote: "string?", label: "string?" },
};

/** An anchor from a note's JSON, checked; throws OfficeError (anchor_kind, anchor_shape). */
export function readAnchor(v: unknown): Anchor {
  if (!v || typeof v !== "object" || Array.isArray(v)) throw new OfficeError("anchor_shape", `an anchor is an object with a "kind", not ${JSON.stringify(v)}`);
  const o = v as Record<string, unknown>;
  const kind = o.kind;
  if (typeof kind !== "string" || !(ANCHOR_KINDS as readonly string[]).includes(kind)) {
    throw new OfficeError(
      "anchor_kind",
      `this bise does not know anchors of kind ${JSON.stringify(kind)} (it knows ${ANCHOR_KINDS.join(", ")}): update bise, or work from the note's text and quote`,
    );
  }
  for (const [field, want] of Object.entries(FIELDS[kind as AnchorKind])) {
    const val = o[field];
    const optional = want.endsWith("?");
    if (val === undefined && optional) continue;
    if (!fits(val, want.replace("?", ""))) {
      throw new OfficeError("anchor_shape", `a ${kind} anchor needs ${field} as ${describe(want)}, got ${JSON.stringify(val)}`);
    }
  }
  return o as Anchor;
}

/** The anchor checked for this file type; throws anchor_kind_mismatch for another type's. */
export function anchorFor(file: FileKind, v: unknown): Anchor {
  const a = readAnchor(v);
  if (!KINDS_OF[file].includes(a.kind)) {
    throw new OfficeError("anchor_kind_mismatch", `a ${a.kind} anchor does not point into a ${file} (a ${file} takes ${KINDS_OF[file].join(" or ")})`);
  }
  return a;
}

function fits(v: unknown, want: string): boolean {
  if (want === "string") return typeof v === "string";
  if (want === "count") return typeof v === "number" && Number.isInteger(v) && v >= 1;
  if (want === "seconds") return typeof v === "number" && Number.isFinite(v) && v >= 0;
  if (want === "rect") {
    if (!v || typeof v !== "object") return false;
    const r = v as Record<string, unknown>;
    return ["x", "y", "w", "h"].every((k) => typeof r[k] === "number" && Number.isFinite(r[k]));
  }
  return false;
}

function describe(want: string): string {
  const w = want.replace("?", "");
  return w === "count" ? "a whole number from 1" : w === "rect" ? "{x, y, w, h} in % of the frame" : w === "seconds" ? "seconds from the start, 0 or more" : "a string";
}
