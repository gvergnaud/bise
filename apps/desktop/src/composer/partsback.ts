// His sent parts back as the composer's segments (the TUI's chips_back.rs): a queued message he
// edits comes back into the field with its chips, never as the hub's rendered text. The inverse of
// message.ts buildMessage for the kinds it makes; a kind it never makes (a page's notes, a hint)
// comes back as its words. Pure.
import type { Part } from '@/proto';
import type { Chip, Segment } from './message';

/** An image's or a file's chip number from its label ('[Image #2]' → 2), null when it has none. Pure. */
export function labelN(name: string): number | null {
  const m = /#(\d+)/u.exec(name)?.[1];
  return m === undefined ? null : Number(m);
}

/** The segments of his parts, chips numbered as sent (a part with no number gets the next one). Pure. */
export function segmentsOfParts(parts: readonly Part[]): Segment[] {
  const out: Segment[] = [];
  let next = 1 + Math.max(0, ...parts.map((p) => ('n' in p && typeof p.n === 'number' ? p.n : 0)));
  const num = (n: number | null | undefined): number => (typeof n === 'number' ? n : next++);
  const chip = (c: Chip): void => {
    out.push({ kind: 'chip', chip: c });
  };
  const words = (t: string): void => {
    const last = out.at(-1);
    if (last?.kind === 'text') out[out.length - 1] = { kind: 'text', text: last.text + t };
    else if (t !== '') out.push({ kind: 'text', text: t });
  };
  for (const p of parts) {
    switch (p.kind) {
      case 'text':
        words(p.text);
        break;
      case 'quote': {
        const at = p.file === undefined ? undefined : { file: p.file, new: p.new ?? '', old: p.old ?? '' };
        chip({ kind: 'quote', n: num(p.n), text: p.text, from: p.from, ...(at === undefined ? {} : { at }) });
        break;
      }
      case 'paste':
        chip({ kind: 'paste', n: p.n, text: p.text });
        break;
      case 'image':
        chip({ kind: 'image', n: num(labelN(p.name)), path: p.source, name: p.source.split('/').at(-1) ?? p.name });
        break;
      case 'file':
        chip({ kind: 'file', n: num(labelN(p.name ?? '')), path: p.path });
        break;
      case 'anchor':
        chip({ kind: 'anchor', n: num(p.n), artifact: p.artifact, version: p.version, ...(p.rev === undefined ? {} : { rev: p.rev }), anchor: p.anchor, ...(p.quote === undefined ? {} : { quote: p.quote }), file: p.artifact });
        break;
      case 'page':
        words(p.text);
        break;
      // a plugin he picked with @ (at-plugins): its chip again
      case 'plugin':
        chip({ kind: 'plugin', n: next++, name: p.name });
        break;
      case 'hint':
        break;
    }
  }
  return out;
}

/** How many images and other attachments his parts carry (the drawer's ' · 1 image'). Pure. */
export function attachedCounts(parts: readonly Part[] | undefined): { readonly images: number; readonly others: number } {
  let images = 0;
  let others = 0;
  for (const p of parts ?? []) {
    if (p.kind === 'image') images += 1;
    else if (p.kind === 'file' || p.kind === 'paste' || p.kind === 'quote' || p.kind === 'anchor') others += 1;
  }
  return { images, others };
}
