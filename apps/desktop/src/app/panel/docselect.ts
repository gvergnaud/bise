// src/app/panel · his selection in a document's view (dev.bise/selection, docs/plugin-apps.md A5) to
// the composer about it: the view's opaque anchor becomes a note anchor `view {key, quote?}` on the
// artifact's version, and his typed key (the view's keys never reach the window) starts his
// message, as kit/office/select.js's typed() does for today's pages (art-notes, frameNote.ts); his
// click on the kit's pill (`take`) attaches it with no key. A selection that only moved: nothing goes. A ~/bise file has no notes this round (A5).
// Pure but for the window event (useFrameNote.ts sendAnchorAct). Owner: apps-host.
import type { FrameNote } from '@/artifacts/frameNote';
import { sendAnchorAct } from '@/artifacts/useFrameNote';
import type { DocInfo, Selection } from '@/views/doc';
import type { DocRef } from '@/views/port';

/** The composer's note for a selection he asked about, or null (it only moved, no anchor, a file, no version). Pure. */
export function noteOf(r: DocRef, s: Selection, d: DocInfo): FrameNote | null {
  const t = r.target;
  if (t.kind !== 'artifact' || s.anchor === null || (s.key === undefined && s.take !== true) || d.version === undefined) return null;
  const anchor = { kind: 'view' as const, key: s.anchor, ...(s.text === undefined ? {} : { quote: s.text }), ...(s.label === undefined ? {} : { label: s.label }) };
  return { artifact: t.id, version: d.version, anchor, ...(s.text === undefined ? {} : { quote: s.text }), ...(s.key === undefined ? {} : { key: s.key }) };
}

/** A selection from a document's view: when he typed over it, the composer about the artifact takes it. */
export function takeSelection(r: DocRef, s: Selection, d: DocInfo): void {
  const note = noteOf(r, s, d);
  const t = r.target;
  if (note === null || t.kind !== 'artifact') return;
  sendAnchorAct({ project: r.project, id: t.id, note, file: d.name });
}
