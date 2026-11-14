// An answer's words as markdown (bar ⛔4: the whole answer readable, code
// keeps its shape, links open, ⌘C copies). Owner: amb-win.
//
// React elements from react-markdown + remark-gfm (tables, task lists,
// autolinks, strikethrough), never innerHTML; raw HTML in an answer shows
// as text. Memo'd on its text: a row re-renders only when its entry's
// words change, never because another entry came.
//
// A link never navigates the window: a click goes to `onLink` (the thread
// hands it the shell's open-in-browser), and only http(s) and mailto links
// are links at all. An `artifact:<id>[@vN]` link is the artifact's name,
// pink inline (ArtName, desktop-agents-bar §5): a click goes to `onArtifact`; resting on it
// shows its preview where the thread provides one (peek/ArtPeek).
import { memo, useEffect, useState, type MouseEvent, type ReactNode } from 'react';
import ReactMarkdown, { defaultUrlTransform, type Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { pathOf } from './paths';
import { Code, PlainCode, known } from './Code';
import { fade, type FadeOpts } from './fade';
import { streamView } from './stream';
import { AppIcon, ArtName, Kbd, Press } from '@/ds';
import { useModHeld } from '@/app/held';
import { ArtPeek } from './peek/ArtPeek';

/** A link he may open from an answer: http, https or mailto. */
export function openable(href: string | undefined): href is string {
  return href !== undefined && /^(https?:|mailto:)/i.test(href);
}

/** The artifact an `artifact:<id>[@vN]` link names: its id, else undefined. Pure. */
export function artifactOf(href: string | undefined): string | undefined {
  const m = href === undefined ? null : /^artifact:([^@\s]+)(?:@v?\d+)?$/u.exec(href);
  return m === null ? undefined : m[1];
}

/** The hrefs react-markdown keeps: its safe ones, and an artifact's. Pure. */
export function keptUrl(url: string): string {
  return artifactOf(url) === undefined ? defaultUrlTransform(url) : url;
}

const plugins = [remarkGfm];

/** The bits of an mdast node the breaks plugin reads. */
interface MdNode {
  type: string;
  value?: string;
  children?: MdNode[];
}

/** Every newline in a text node becomes a line break (remark-breaks' rule): code is left as written. Pure. */
export function hardBreaks(node: MdNode): void {
  if (node.children === undefined) return;
  const out: MdNode[] = [];
  for (const c of node.children) {
    if (c.type === 'text' && c.value !== undefined && c.value.includes('\n')) {
      c.value.split('\n').forEach((part, i) => {
        if (i > 0) out.push({ type: 'break' });
        if (part !== '') out.push({ type: 'text', value: part });
      });
    } else {
      hardBreaks(c);
      out.push(c);
    }
  }
  node.children = out;
}

// his messages keep every line he typed (shift+enter), as the TUI shows them (qa-flows 68b4f1ea)
const withBreaks = [remarkGfm, () => hardBreaks];

/** A chip's place in his words (YouBody): private-use marks around its index. Pure. */
export const slotMark = (i: number): string => `\uE000${String(i)}\uE001`;
/** His own words with the marks' chars escaped (an icon font's glyph he pasted), so no text of his
 *  ever reads as a slot (architect m_29179); slotNodes puts them back. Pure. */
export const slotText = (text: string): string => text.replace(/[\uE000-\uE002]/gu, (c) => `\uE002${c}`);
const SLOT = /(?<!\uE002)\uE000(\d+)\uE001/gu;
const unescape = (text: string): string => text.replace(/\uE002([\uE000-\uE002])/gu, '$1');

/** Each slot mark in a text node becomes a slot span (drawn by Markdown's `slot`); in code, `label`
 *  writes it as words, so code keeps its shape; his own escaped chars come back (slotText). Pure
 *  (mutates the tree it is given). */
export function slotNodes(node: MdNode & { data?: unknown }, label: (i: number) => string): void {
  if (node.children === undefined) return;
  const out: MdNode[] = [];
  for (const c of node.children) {
    if ((c.type === 'inlineCode' || c.type === 'code') && c.value !== undefined) {
      c.value = unescape(c.value.replace(SLOT, (_, i: string) => label(Number(i))));
      out.push(c);
    } else if (c.type === 'text' && c.value !== undefined) {
      let at = 0;
      for (const m of c.value.matchAll(SLOT)) {
        if (m.index > at) out.push({ type: 'text', value: unescape(c.value.slice(at, m.index)) });
        out.push({ type: 'slot', data: { hName: 'span', hProperties: { dataSlot: m[1] } } } as MdNode);
        at = m.index + m[0].length;
      }
      if (at < c.value.length) out.push({ type: 'text', value: unescape(c.value.slice(at)) });
    } else {
      slotNodes(c, label);
      out.push(c);
    }
  }
  node.children = out;
}

/** A code block's language from its class ('language-rust'). Pure. */
export function langOf(className: unknown): string | undefined {
  const list = Array.isArray(className) ? className : typeof className === 'string' ? className.split(' ') : [];
  for (const c of list) if (typeof c === 'string' && c.startsWith('language-')) return c.slice(9);
  return undefined;
}

/** The text of a markdown tree node, as written. Pure. */
export function textOf(n: { type: string; value?: unknown; children?: readonly unknown[] }): string {
  if (n.type === 'text' && typeof n.value === 'string') return n.value;
  return (n.children ?? []).map((c) => textOf(c as { type: string })).join('');
}

export const Markdown = memo(function Markdown({
  text,
  onLink,
  onArtifact,
  onPath,
  breaks = false,
  fading,
  slot,
  slotLabel,
}: {
  text: string;
  /** a link's click; `mod`: ⌘ (⌃) held, his browser (page newtab-browser) */
  onLink?: (url: string, mod?: boolean) => void;
  /** an `artifact:` link's click: the artifact's view */
  onArtifact?: (id: string) => void;
  onPath?: (path: string, line?: number) => void;
  /** every line he typed stays a line (his own messages); answers stay standard markdown */
  breaks?: boolean;
  /** a streamed answer: its new words fade in (Answer below) */
  fading?: FadeOpts;
  /** his words' chips (YouBody): what draws the i-th slot mark (slotMark), and its words in code */
  slot?: (i: number) => ReactNode;
  slotLabel?: (i: number) => string;
}) {
  const label = slotLabel ?? (() => '');
  const remark = slot === undefined ? (breaks ? withBreaks : plugins) : [...(breaks ? withBreaks : plugins), () => (tree: MdNode) => { slotNodes(tree, label); }];
  const components: Components = {
    span({ node, children, ...rest }) {
      const at = node?.properties.dataSlot;
      if (slot !== undefined && (typeof at === 'string' || typeof at === 'number')) return slot(Number(at));
      return <span {...rest}>{children}</span>;
    },
    // a file path in inline code opens in his editor (tui-parity M4, B14);
    // a block's code (it ends with a newline) stays code
    code({ className, children }) {
      const t = typeof children === 'string' ? children : '';
      const f = onPath === undefined || t.includes('\n') ? undefined : pathOf(t);
      if (f === undefined) return <code className={className}>{children}</code>;
      return (
        <code
          className="th-path"
          role="link"
          tabIndex={0}
          title={`open ${f.path} in your editor`}
          onClick={() => {
            onPath?.(f.path, f.line);
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') onPath?.(f.path, f.line);
          }}
          data-testid="thread.path"
        >
          {children}
        </code>
      );
    },
    a({ node, href, children }) {
      const art = artifactOf(href);
      if (art !== undefined) {
        // resting on it shows its preview (peek/ArtPeek; only where the thread provides one)
        return (
          <ArtPeek id={art} name={node === undefined ? '' : textOf(node)}>
          <ArtName
            onClick={
              onArtifact === undefined
                ? undefined
                : () => {
                    onArtifact(art);
                  }
            }
            testId="thread.artname"
          >
            {/* the pink chip (QA §4): its 14 line icon, then its name */}
            <AppIcon name="artifacts" size={14} />
            {children}
          </ArtName>
          </ArtPeek>
        );
      }
      if (!openable(href)) return <span className="th-link-off">{children}</span>;
      const open = (e: MouseEvent) => {
        e.preventDefault();
        onLink?.(href, e.metaKey || e.ctrlKey);
      };
      return (
        <a href={href} onClick={open} title={href} rel="noreferrer" data-testid="thread.link">
          {children}
        </a>
      );
    },
    // a block keeps its shape: no wrapping, its own horizontal scroll (in his bubble it wraps,
    // thread.css); its language on top, a copy button on hover (tui-parity B15, M8)
    pre({ node, children }) {
      const code = node?.children.find((c) => c.type === 'element' && c.tagName === 'code');
      const lang = code?.type === 'element' ? langOf(code.properties.className) : undefined;
      const text = code === undefined ? '' : textOf(code);
      // a streamed answer: its new box fades in once, then its new whole lines (fade.ts)
      const fresh = code?.type === 'element' && typeof code.properties.dataFrom === 'string' ? Number(code.properties.dataFrom) : undefined;
      const boxIn = Array.isArray(node?.properties.className) && node.properties.className.includes('th-in');
      const bare = breaks && lang === undefined;
      return (
        <div className={boxIn ? 'th-code th-in' : 'th-code'}>
          <div className="th-code-head">
            <span className="th-code-lang b-md-lang">{lang ?? ''}</span>
            <CodeCopy text={text} />
          </div>
          {/* his words (breaks): a bare fence's block gray, as his composer draws it (code-fences) */}
          <pre className="th-pre">{bare || known(lang) ? <Code text={text} lang={lang ?? ''} from={fresh} /> : fresh !== undefined ? <PlainCode text={text} from={fresh} /> : children}</pre>
        </div>
      );
    },
    table({ children }) {
      return (
        <div className="th-table">
          <table>{children}</table>
        </div>
      );
    },
    img({ alt }) {
      // an answer's image link is not fetched here: its words stand in
      return <span className="th-img">{alt ?? 'image'}</span>;
    },
  };
  return (
    // b-md: the ds's markdown colors, the TUI's (src/ds/css/md.css); th-md: the thread's layout
    <div className="th-md b-md">
      <ReactMarkdown remarkPlugins={remark} rehypePlugins={fading === undefined ? undefined : [[fade, fading]]} components={components} urlTransform={keptUrl} skipHtml>
        {text}
      </ReactMarkdown>
    </div>
  );
});

/** Each Answer's rendered length after its last pass (its next update fades from there). */
/** A code block's copy, on hover; ctrl held, '⌃Y copy' (ctrl+y copies the block under the mouse,
 *  ThreadScreen codeToCopy; the TUI's ctrlhint.rs key bar 'ctrl+y copy code'). */
function CodeCopy({ text }: { text: string }) {
  const ctrl = useModHeld('Control');
  return (
    <Press
      look="th-code-copy"
      onClick={() => {
        void (window as Partial<Window>).bise?.copy(text);
      }}
      data-testid="thread.code.copy"
    >
      {ctrl && (
        <span className="app-keyhint" data-testid="thread.code.hint">
          <Kbd>⌃Y</Kbd>
        </span>
      )}{' '}
      copy
    </Press>
  );
}

const totals = new WeakMap<object, number>();

/** How long the new words of one update take to fade in (300 ms of spread + 180 ms of fade, R40). */
export const FADE_MS = 480;

/**
 * An agent's answer, streamed or whole (text-appearing v2): while `live`,
 * only what is whole shows (stream.ts) and the working sign rides after the
 * last word; at each new text, only the words new since the last update
 * fade in, and FADE_MS later the thread draws them plain again (nothing
 * above re-animates or moves). The entry that replaces the live text is the
 * same Answer (the same row): its last words fade in, nothing else moves.
 * `still`: no fade (reduced motion, or the window not in front); the text
 * still comes with each update.
 */
export function Answer({
  text,
  live = false,
  still = false,
  onLink,
  onArtifact,
  onPath,
}: {
  text: string;
  live?: boolean;
  still?: boolean;
  /** a link's click; `mod`: ⌘ (⌃) held, his browser (page newtab-browser) */
  onLink?: (url: string, mod?: boolean) => void;
  onArtifact?: (id: string) => void;
  onPath?: (path: string, line?: number) => void;
}) {
  const shown = live ? streamView(text) : text;
  // the text's rendered length after each pass (fade.ts onTotal): the next update's `from`
  const [me] = useState(() => ({}));
  const [seen, setSeen] = useState(shown);
  // Infinity: nothing fades (a row drawn whole, or its words already faded)
  const [from, setFrom] = useState(Infinity);
  if (shown !== seen) {
    setSeen(shown);
    setFrom(still ? Infinity : (totals.get(me) ?? Infinity));
  }
  useEffect(() => {
    if (from === Infinity) return;
    const t = window.setTimeout(() => {
      setFrom(Infinity);
    }, FADE_MS);
    return () => {
      window.clearTimeout(t);
    };
  }, [from, seen]);
  const fading: FadeOpts | undefined =
    live || from !== Infinity
      ? {
          from,
          wave: live,
          onTotal: (n) => {
            totals.set(me, n);
          },
        }
      : undefined;
  return (
    <div className={still ? 'th-answer th-still' : 'th-answer'} data-live={live || undefined}>
      <Markdown text={shown} onLink={onLink} onArtifact={onArtifact} onPath={onPath} fading={fading} />
    </div>
  );
}
