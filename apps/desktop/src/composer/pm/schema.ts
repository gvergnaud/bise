// The field's schema as TipTap extensions: one paragraph of inline nodes (pmtext.ts reads it).
//   - text, no marks: markdown stays characters (the wire is the TUI's text); its colors are
//     decorations, never marks, so a color never changes the document;
//   - hardBreak: a newline (⇧⏎, a pasted line) as one position, the old engine's line break;
//   - chip: an image, a long paste, a quote, an artifact; an inline atom, one caret step, one
//     backspace; its data (message.ts Chip) is the node's `chip` attribute;
//   - dictation: where his words go while he talks, showing them live; read by nothing (not the
//     send, not the draft), replaced by the words when the last ones come.
// The node views are plain DOM (no React portal), drawn once per node: typing never renders React.
import { Node } from '@tiptap/core';
import { appIconPaths, iconKindOf, kindIconPaths, type AppIconName } from '@/ds';
import { chipLabel, fileChipName, type Chip } from '../message';
import './chip.css';
import { BARS, fitFor, flat, push, SAMPLE_MS, timeOf, wave, WAVE_MS } from './meter';

export const Doc = Node.create({ name: 'doc', topNode: true, content: 'paragraph' });

export const Paragraph = Node.create({
  name: 'paragraph',
  group: 'block',
  content: 'inline*',
  parseHTML: () => [{ tag: 'p' }],
  renderHTML: () => ['p', { class: 'p' }, 0],
});

export const Text = Node.create({ name: 'text', group: 'inline' });

export const HardBreak = Node.create({
  name: 'hardBreak',
  group: 'inline',
  inline: true,
  selectable: false,
  parseHTML: () => [{ tag: 'br' }],
  renderHTML: () => ['br'],
  renderText: () => '\n',
});

/** A chip's DOM, the old engine's (composer.chip.<n>, data-kind, the image's thumbnail). The one
 *  inline chip: his sent bubble draws the same DOM in his words (thread/YouBody InlineChip, its
 *  look in chip.css), with its own test id. */
export function chipDom(c: Chip, testId = `composer.chip.${c.n}`): HTMLElement {
  const el = document.createElement('span');
  el.className = `b-chip b-inchip ${c.kind}`;
  el.dataset.testid = testId;
  el.dataset.kind = c.kind;
  el.dataset.n = String(c.n);
  el.contentEditable = 'false';
  if (c.kind === 'image') {
    // the attachment list's number beside the 16 px thumbnail (else the no-file glyph slot), its
    // name in the list's row (designer's composer-attachments A); '[Image #1]' stays the copied text
    el.title = c.name;
    el.append(c.thumb !== undefined ? thumbImg(c.thumb) : noFileGlyph());
    el.append(String(c.n));
    return el;
  }
  if (c.kind === 'file') {
    // '▯ e2e.mjs' (the file's 16 px system icon, else ▯, and its name: designer m_27881; its
    // number, size and folder in the list's row); '[File #3]' is the copied text
    el.title = c.path;
    el.append(c.icon !== undefined ? thumbImg(c.icon) : fileGlyph());
    el.append(fileChipName(c.path));
    return el;
  }
  if (c.kind === 'artifact' || c.kind === 'plugin') {
    // the @ list's icon for it (at-menu-icons, designer's C), 14 px in text, then its words;
    // '↗ title' / '⊞ name' stay the copied text (chipLabel)
    el.title = c.kind === 'artifact' ? c.title : c.name;
    el.append(c.kind === 'plugin' ? chipIcon(appIconPaths('plugins'), 24) : artifactIcon(c.what ?? '', c.url));
    el.append(c.kind === 'artifact' ? c.title : c.name);
    return el;
  }
  el.append(chipLabel(c));
  return el;
}

/** An artifact chip's icon: its kind's (ds KindIcon), a folder of artifacts the 'artifacts' stack. */
function artifactIcon(what: string, url: string): Element {
  if (what.toLowerCase() === 'folder') return chipIcon(appIconPaths('artifacts' satisfies AppIconName), 24);
  const kind = iconKindOf({ kind: what, url });
  const svg = chipIcon(kindIconPaths(kind), 16);
  svg.dataset.kind = kind;
  return svg;
}

/** A line icon in a chip, 14 px on its `grid`, stroke 1.5 in currentColor (the ds icons' look). */
function chipIcon(paths: readonly string[], grid: number): SVGSVGElement {
  const ns = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(ns, 'svg');
  svg.setAttribute('class', 'icon');
  svg.setAttribute('viewBox', `0 0 ${String(grid)} ${String(grid)}`);
  svg.setAttribute('width', '14');
  svg.setAttribute('height', '14');
  svg.setAttribute('aria-hidden', 'true');
  for (const [k, v] of Object.entries({ fill: 'none', stroke: 'currentColor', 'stroke-width': '1.5', 'stroke-linecap': 'round', 'stroke-linejoin': 'round' })) svg.setAttribute(k, v);
  for (const d of paths) {
    const p = document.createElementNS(ns, 'path');
    p.setAttribute('d', d);
    p.setAttribute('vector-effect', 'non-scaling-stroke');
    svg.append(p);
  }
  return svg;
}

/** A file chip's glyph slot when the system gives no icon: ▯, 16 px (the TUI's G_FILE). */
export function fileGlyph(): HTMLElement {
  const span = document.createElement('span');
  span.className = 'thumb file';
  span.setAttribute('aria-hidden', 'true');
  span.textContent = '▯';
  return span;
}

/** An image chip's words on the input side: 'image 1', as his sent bubble shows it. Pure. */
export const imageChipWords = (n: number): string => `image ${String(n)}`;

function thumbImg(src: string): HTMLElement {
  const img = document.createElement('img');
  img.className = 'thumb';
  img.src = src;
  img.alt = '';
  img.draggable = false;
  // a file gone since (a bubble's image by its path): the no-file slot, same 16 px
  img.onerror = () => {
    img.replaceWith(noFileGlyph());
  };
  return img;
}

/** The no-file glyph slot of the thread's Thumb (rows-cards.tsx): a faint image outline, 16 px. */
function noFileGlyph(): HTMLElement {
  const span = document.createElement('span');
  span.className = 'thumb none';
  span.setAttribute('aria-hidden', 'true');
  const ns = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(ns, 'svg');
  svg.setAttribute('viewBox', '0 0 16 16');
  svg.setAttribute('width', '16');
  svg.setAttribute('height', '16');
  const shapes: [string, Record<string, string>][] = [
    ['rect', { x: '2.5', y: '3.5', width: '11', height: '9', rx: '1.5', fill: 'none', stroke: 'currentColor' }],
    ['circle', { cx: '6', cy: '6.5', r: '1', fill: 'currentColor' }],
    ['path', { d: 'M3 11.5l3.5-3 2.5 2 2-1.5 2.5 2', fill: 'none', stroke: 'currentColor' }],
  ];
  for (const [tag, attrs] of shapes) {
    const s = document.createElementNS(ns, tag);
    for (const [k, v] of Object.entries(attrs)) s.setAttribute(k, v);
    svg.append(s);
  }
  span.append(svg);
  return span;
}

export const ChipNode = Node.create({
  name: 'chip',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: false,
  draggable: false,
  addAttributes: () => ({ chip: { default: null, rendered: false } }),
  renderHTML: ({ node }) => ['span', { class: 'b-chip' }, chipLabel(node.attrs.chip as Chip)],
  renderText: ({ node }) => chipLabel(node.attrs.chip as Chip),
  addNodeView: () => ({ node }) => ({ dom: chipDom(node.attrs.chip as Chip), ignoreMutation: () => true }),
});

/** The live words after the take's chip (none before the first: the chip says it listens; a
 *  batch listener gives every word at the end, for as long as it takes). */
const wordsOf = (n: { attrs: Record<string, unknown> }): string => (n.attrs.text as string).trim();

/** A take that came back with no words or an error, the core keeping it (designer m_22396): its line. */
export const FAILED = {
  noWords: "didn't catch any words",
  error: (reason: string): string => `couldn't transcribe it${reason === '' ? '' : ` (${reason})`}`,
  retry: 'try again',
  retryKey: '⌥⏎',
  drop: 'esc drops it',
} as const;
/** The core's error, short for the line: no final period. Pure. */
export const shortReason = (error: string | null): string => (error ?? '').trim().replace(/\.$/u, '');
/** The node's 'try again' asks its take again through this event (dictation.ts listens), its detail the take's id. */
export const RETRY_EVENT = 'b-dictate-retry';

const el = (tag: string, cls: string, text: string): HTMLElement => {
  const e = document.createElement(tag);
  if (cls !== '') e.className = cls;
  e.textContent = text;
  return e;
};

/** The failed take's line: '▲ couldn't transcribe it (…)' or 'didn't catch any words', then
 *  'try again ⌥⏎' (a click does it) and 'esc drops it'. */
function failedLine(dom: HTMLElement, id: string, lead: string, reason: string | null): void {
  const parts: (HTMLElement | string)[] = [lead];
  if (reason !== null) parts.push(el('span', 'warn', '▲'), ' ');
  parts.push(el('span', 'w', reason === null ? FAILED.noWords : FAILED.error('')));
  if (reason !== null && reason !== '') {
    // the reason is the part that cuts (fitFailed): '(the transcription ti…)', then none
    const why = el('span', 'why', ` (${reason})`);
    why.dataset.full = reason;
    parts.push(why);
  }
  const retry = el('span', 'retry', FAILED.retry);
  retry.setAttribute('role', 'button');
  retry.dataset.testid = 'composer.dictation.retry';
  // the field keeps its caret; the click asks the take again
  retry.addEventListener('mousedown', (e) => {
    e.preventDefault();
  });
  retry.addEventListener('click', (e) => {
    e.preventDefault();
    retry.dispatchEvent(new CustomEvent(RETRY_EVENT, { bubbles: true, detail: id }));
  });
  // '· try again ⌥⏎ · esc drops it' never cuts or wraps (designer m_22494)
  const tail = el('span', 'tail', ' · ');
  tail.append(retry, ' ', el('span', 'k', FAILED.retryKey), ' · ', el('span', 'k-what', FAILED.drop));
  parts.push(tail);
  dom.replaceChildren(...parts);
}

/** A failed take's line stays one line (designer m_22494): its reason cuts with … first, then
 *  drops whole. Measured where it sits (the words typed before it move it), so after each change
 *  of the field and each change of its width. No layout (jsdom): left whole. */
export function fitFailed(dom: HTMLElement): void {
  const why = dom.querySelector<HTMLElement>('.why');
  const full = why?.dataset.full;
  if (why === null || full === undefined) return;
  const fits = (): boolean => dom.getClientRects().length <= 1;
  const cut = (n: number): string => ` (${full.slice(0, n).trimEnd()}…)`;
  why.textContent = ` (${full})`;
  if (fits()) return;
  // the longest cut that fits (at least one letter), else no reason at all
  let lo = 1;
  let hi = full.length - 1;
  let best = 0;
  while (lo <= hi) {
    const n = Math.floor((lo + hi) / 2);
    why.textContent = cut(n);
    if (fits()) {
      best = n;
      lo = n + 1;
    } else hi = n - 1;
  }
  why.textContent = best > 0 ? cut(best) : '';
}
/** Every failed line in the field, fitted again. */
export const fitFailedIn = (root: HTMLElement): void => {
  root.querySelectorAll<HTMLElement>('.b-dictation[data-failed]').forEach(fitFailed);
};

/** His voice level (0-1) while the mic is on: called now and on each change; returns the unsubscribe. */
export type LevelSource = (fn: (v: number) => void) => () => void;

/** The take's chip, the TUI's voice chip (meter.ts, designer m_22700): listening, '●' blinking,
 *  the last 6 levels of his voice and the time; transcribing, a steady '∿', the rolling wave and
 *  the clip's length. Under reduced motion the ● stays lit and the wave holds its first frame;
 *  the bars still follow his voice (data, as the TUI's). `level` is read when the mic is on (the
 *  field's sources live in a ref). The time is a CSS content (data-t): the node's text is his
 *  words alone. */
function liveChip(level: () => LevelSource | undefined): {
  readonly chip: HTMLElement;
  readonly phase: (wait: boolean) => void;
  /** narrow: the timer drops, then 3 bars, so `fieldW` keeps 160 px of free text */
  readonly fit: (fieldW: number) => void;
  readonly stop: () => void;
} {
  const chip = document.createElement('span');
  chip.className = 'chip';
  chip.setAttribute('role', 'img');
  const sign = document.createElement('i');
  sign.className = 'sign';
  const meter = document.createElement('span');
  meter.className = 'mt';
  meter.dataset.testid = 'composer.dictation.meter';
  const bars = Array.from({ length: BARS }, () => document.createElement('i'));
  meter.append(...bars);
  const timer = document.createElement('span');
  timer.className = 't';
  timer.dataset.t = timeOf(0);
  chip.append(sign, meter, timer);
  const reduced = typeof window.matchMedia === 'function' && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const fit = (fieldW: number): void => {
    const f = fitFor(fieldW, (k) => {
      chip.dataset.fit = k;
      return chip.getBoundingClientRect().width;
    });
    chip.dataset.fit = f;
  };
  const show = (steps: readonly number[]): void => {
    const s = steps.join(',');
    if (meter.dataset.bars === s) return;
    meter.dataset.bars = s;
    steps.forEach((h, i) => {
      bars[i]?.style.setProperty('--h', String(h));
    });
  };
  let now: 'listening' | 'transcribing' | null = null;
  let clock: ReturnType<typeof setInterval> | null = null;
  let off: (() => void) | null = null;
  // at rest (a failed line, the node gone): the next phase starts from its start
  const stop = (): void => {
    if (clock !== null) clearInterval(clock);
    clock = null;
    off?.();
    off = null;
    now = null;
  };
  const phase = (wait: boolean): void => {
    const next = wait ? 'transcribing' : 'listening';
    if (next === now) return;
    stop();
    now = next;
    chip.dataset.phase = next;
    meter.dataset.phase = next;
    chip.setAttribute('aria-label', next);
    if (next === 'listening') {
      // one level each 100 ms, the newest on the right; silence (or no level at all) stays flat;
      // the time counts from here, and holds once the mic is off
      let last = 0;
      let steps = flat();
      const from = Date.now();
      show(steps);
      off = level()?.((v) => {
        last = v;
      }) ?? null;
      clock = setInterval(() => {
        steps = push(steps, last);
        show(steps);
        const t = timeOf(Date.now() - from);
        if (timer.dataset.t !== t) timer.dataset.t = t;
      }, SAMPLE_MS);
      return;
    }
    let n = 0;
    show(wave(0));
    if (reduced) return;
    clock = setInterval(() => {
      n += 1;
      show(wave(n));
    }, WAVE_MS);
  };
  return { chip, phase, fit, stop };
}

export const DictationNode = Node.create<{ level: () => LevelSource | undefined }>({
  name: 'dictation',
  addOptions: () => ({ level: () => undefined }),
  group: 'inline',
  inline: true,
  atom: true,
  selectable: false,
  draggable: false,
  // id: its take (a new take may start while an older one is still transcribed); wait: the mic is
  // off; failed: its final had no words or an error and the core kept it ('try again'), reason:
  // the error, short ('': no words)
  addAttributes: () => ({
    text: { default: '', rendered: false },
    lead: { default: '', rendered: false },
    id: { default: '', rendered: false },
    wait: { default: false, rendered: false },
    failed: { default: false, rendered: false },
    reason: { default: '', rendered: false },
  }),
  renderHTML: () => ['span', { class: 'b-dictation' }],
  renderText: () => '',
  addNodeView() {
    const { level } = this.options;
    return ({ node }) => {
      const dom = document.createElement('span');
      dom.className = 'b-dictation';
      dom.dataset.testid = 'composer.dictation';
      dom.contentEditable = 'false';
      const live = liveChip(level);
      // the lead (a space after a word) stays before the chip: the words read on from his text
      const lead = document.createTextNode('');
      const words = document.createElement('span');
      words.className = 'w';
      let failed = false;
      // narrow: the chip fits the field's width (in the page by the next frame; no layout in jsdom)
      let sizes: ResizeObserver | null = null;
      requestAnimationFrame(() => {
        const box = dom.closest<HTMLElement>('.b-field');
        if (box === null || typeof ResizeObserver !== 'function') return;
        sizes = new ResizeObserver(() => {
          if (!failed) live.fit(box.clientWidth);
        });
        sizes.observe(box);
      });
      const draw = (n: typeof node): void => {
        failed = n.attrs.failed === true;
        dom.toggleAttribute('data-failed', failed);
        if (failed) {
          live.stop();
          failedLine(dom, n.attrs.id as string, n.attrs.lead as string, n.attrs.reason === '' ? null : (n.attrs.reason as string));
          // in the page by the next frame: fitted where it sits
          requestAnimationFrame(() => {
            fitFailed(dom);
          });
        }
        else {
          lead.textContent = n.attrs.lead as string;
          words.textContent = wordsOf(n);
          dom.replaceChildren(lead, live.chip, words);
          live.phase(n.attrs.wait === true);
        }
      };
      draw(node);
      return {
        dom,
        ignoreMutation: () => true,
        // a new partial changes only the words' text: the node's DOM stays the same node
        update: (next) => {
          if (next.type.name !== 'dictation') return false;
          if (failed || next.attrs.failed === true) {
            draw(next);
            return true;
          }
          const w = wordsOf(next);
          if (words.textContent !== w) words.textContent = w;
          live.phase(next.attrs.wait === true);
          return true;
        },
        destroy: () => {
          live.stop();
          sizes?.disconnect();
        },
      };
    };
  },
});

/** Every node of the field, in one list (the editor's and the tests' schema). */
export const FIELD_NODES = [Doc, Paragraph, Text, HardBreak, ChipNode, DictationNode];
/** The field's nodes, its dictation's meter reading his voice level from `level` (the field's sources). */
export const fieldNodes = (level: () => LevelSource | undefined): typeof FIELD_NODES => [
  Doc,
  Paragraph,
  Text,
  HardBreak,
  ChipNode,
  DictationNode.configure({ level }),
];
