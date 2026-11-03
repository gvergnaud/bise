// ⌘F's bar over an artifact's page (artifact-find, his m_52682): the thread's find bar as it looks
// (thread/FindBar.tsx: a compact panel at the top right, field, count, ↑ ↓ ×), searching the page.
// The page is a pages-server frame the window cannot read: the frame's find agent (main's
// findagent.ts FIND_AGENT) finds, highlights and scrolls; this says the needle and the current
// match ({bise: find, q, at, seq}) and reads the count back ({bise: found, seq, n}). ⏎ next,
// ⇧⏎ previous (around the ends), esc closes and clears the page's highlights; an esc inside the
// page closes it too, before the artifact (claimFindEsc, app/escape.ts frameEsc). `ask` bumps
// when ⌘F comes again while it shows: the field takes the focus back, its words selected.
import { useEffect, useRef, useState, type RefObject } from 'react';
import { Field, IconButton } from '@/ds';
import { panel } from '@/proto/words_panel.gen';
import { FRAME_KINDS, originOf } from './frameNote';
import { claimFindEsc, counterOf, stepAt } from './find';

export interface ArtifactFindBarProps {
  /** the page's frame */
  frame: RefObject<HTMLIFrameElement | null>;
  /** its src: the origin the messages go to and come from */
  src: string;
  /** bumped by each ⌘F while it shows */
  ask: number;
  onClose: () => void;
}

export function ArtifactFindBar({ frame, src, ask, onClose }: ArtifactFindBarProps) {
  const [needle, setNeedle] = useState('');
  const [n, setN] = useState(0);
  const [at, setAt] = useState(-1);
  const seq = useRef(0);
  const field = useRef<HTMLInputElement>(null);
  const origin = originOf(src);

  // to '*': a copy served under the sandbox CSP (a site, a text file) has an opaque origin; the
  // guard keeps every subframe on the pages server, and only the needle goes
  const tell = (q: string, cur: number): void => {
    seq.current += 1;
    const win = frame.current?.contentWindow;
    if (win == null) return;
    win.postMessage({ bise: FRAME_KINDS.find, q, at: cur, seq: seq.current }, '*');
  };
  const tellRef = useRef(tell);
  useEffect(() => {
    tellRef.current = tell;
  });

  // the frame's count for the last thing said; an older answer changes nothing
  useEffect(() => {
    const on = (e: MessageEvent) => {
      const d: unknown = e.data;
      // its own frame only, on the pages server's origin or an opaque one (a sandboxed copy)
      if (e.source == null || e.source !== frame.current?.contentWindow || (e.origin !== 'null' && e.origin.toLowerCase() !== origin)) return;
      if (typeof d !== 'object' || d === null) return;
      const m = d as { bise?: unknown; seq?: unknown; n?: unknown };
      if (m.bise !== FRAME_KINDS.found || m.seq !== seq.current || typeof m.n !== 'number') return;
      const count = Math.max(0, Math.floor(m.n));
      setN(count);
      setAt((x) => (count === 0 ? -1 : Math.min(Math.max(x, 0), count - 1)));
    };
    window.addEventListener('message', on);
    return () => {
      window.removeEventListener('message', on);
    };
  }, [frame, origin]);

  // ⌘F again: the field again, its words selected
  useEffect(() => {
    field.current?.focus();
    field.current?.select();
  }, [ask]);

  const close = (): void => {
    tellRef.current('', -1);
    onClose();
  };
  const closeRef = useRef(close);
  useEffect(() => {
    closeRef.current = close;
  });
  useEffect(() => claimFindEsc(() => { closeRef.current(); }), []);
  // gone (the artifact closed, another version): the page's highlights go with it
  useEffect(
    () => () => {
      tellRef.current('', -1);
    },
    [],
  );

  const step = (back: boolean): void => {
    const next = stepAt(n, at, back);
    setAt(next);
    tell(needle, next);
  };

  return (
    <div className="art-find" data-testid="artifact.find" role="search">
      <Field
        ref={field}
        testId="artifact.find.field"
        placeholder={panel.art_find_placeholder}
        aria-label={panel.art_find_placeholder}
        value={needle}
        onChange={(e) => {
          const q = e.currentTarget.value;
          setNeedle(q);
          setAt(q === '' ? -1 : 0);
          if (q === '') setN(0);
          tell(q, q === '' ? -1 : 0);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            step(e.shiftKey);
          } else if (e.key === 'Escape') {
            e.preventDefault();
            e.stopPropagation();
            close();
          }
        }}
      />
      <span className="art-find-count" data-testid="artifact.find.count">
        {counterOf(needle, n, at)}
      </span>
      <span className="art-find-acts">
        <IconButton label={panel.art_find_prev} keys="⇧⏎" disabled={n === 0} onClick={() => { step(true); }} testId="artifact.find.prev">
          ↑
        </IconButton>
        <IconButton label={panel.art_find_next} keys="⏎" disabled={n === 0} onClick={() => { step(false); }} testId="artifact.find.next">
          ↓
        </IconButton>
        <IconButton label={panel.art_find_close} keys="esc" onClick={close} testId="artifact.find.close">
          ×
        </IconButton>
      </span>
    </div>
  );
}
