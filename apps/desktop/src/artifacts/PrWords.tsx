// A PR's words drawn the one way (designer m_54748): its tag, the part that waits on him in the
// accent first, the rest dim; a ✗ always in err with the words after it in ink; the forge's
// lateness last, its tip saying why. In a row (`fit`), parts with no room drop whole from the end,
// so the pink one stays. Used by the right bar, the Artifacts page's cards and rows, and the PR
// tab's status line. Owner: pr-artifacts.
import type { ReactNode } from 'react';
import type { Artifact } from '@/proto';
import { tipAttrs } from '@/ds';
import { panel } from '@/proto/words_panel.gen';
import { prLine, prPieces } from './pr';
import './prwords.css';

/** A part's text with each ✗ in err, the words after it in ink. */
function marked(text: string): ReactNode {
  const bits = text.split('✗');
  if (bits.length === 1) return text;
  return bits.map((b, i) => (i === 0 ? <span key={i}>{b}</span> : (
    <span key={i}>
      <span className="prw-x">✗</span>
      <span className="prw-ink">{b}</span>
    </span>
  )));
}

export function PrWords({ a, tag = true, fit = false, tail, testId }: {
  a: Pick<Artifact, 'pr'>;
  tag?: boolean;
  fit?: boolean;
  /** a last part, the first to go ('perf', its maker in a row) */
  tail?: string;
  testId?: string;
}) {
  const l = prLine(a);
  if (l === null) return null;
  const parts: { key: string; node: ReactNode; cls: string; tip?: string }[] = [
    ...(tag ? [{ key: 'tag', node: l.sigil, cls: 'prw-tag' }] : []),
    ...prPieces(a).map((p, i) => ({ key: `w${String(i)}`, node: marked(p.text), cls: p.acc ? 'prw-acc' : 'prw-dim' })),
    ...(l.late === '' ? [] : [{ key: 'late', node: l.late, cls: 'prw-late', tip: panel.pr_as_of_tip }]),
    ...(tail === undefined || tail === '' ? [] : [{ key: 'tail', node: tail, cls: 'prw-dim' }]),
  ];
  return (
    <span className={fit ? 'prw prw-fit' : 'prw'} data-testid={testId}>
      {parts.map((p, i) => (
        <span key={p.key} className={p.cls} data-acc={p.cls === 'prw-acc' ? '' : undefined} {...(p.tip === undefined ? {} : tipAttrs(p.tip))}>
          {i > 0 ? ' · ' : ''}
          {p.node}
        </span>
      ))}
    </span>
  );
}
