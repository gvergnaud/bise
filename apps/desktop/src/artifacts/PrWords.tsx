// A PR's words drawn the one way (designer m_54748): its tag, then the hub's typed pieces in the
// hub's order (Pr.pieces: what waits on him first, in the accent; a failing check in err; the
// rest dim), the forge's lateness last, its tip saying why. Toned by their `tone`, never by their
// text (architect m_54755); an older hub's row with no pieces: its words whole. In a row (`fit`),
// parts with no room drop whole from the end, so the pink one stays. Used by the right bar, the
// Artifacts page's cards and rows, and the PR tab's status line. Owner: pr-artifacts.
import type { ReactNode } from 'react';
import type { Artifact, PrTone } from '@/proto';
import { Press, tipAttrs } from '@/ds';
import { panel } from '@/proto/words_panel.gen';
import { prLine } from './pr';
import './prwords.css';

const TONE: Record<PrTone, string> = { yours: 'prw-acc', fail: 'prw-fail', plain: 'prw-dim', unknown: 'prw-dim' };

export function PrWords({ a, tag = true, fit = false, tail, onFail, testId }: {
  a: Pick<Artifact, 'pr'>;
  tag?: boolean;
  fit?: boolean;
  /** a last part, the first to go ('perf', its maker in a row) */
  tail?: string;
  /** a failing piece ('checks ✗ 1') opens what it says (the PR tab's checks: page pr-guided V2) */
  onFail?: () => void;
  testId?: string;
}) {
  const l = prLine(a);
  if (l === null) return null;
  const typed = a.pr?.status?.pieces ?? [];
  const words = typed.length > 0
    ? typed.map((p, i) => ({ key: `w${String(i)}`, node: p.text as ReactNode, cls: TONE[p.tone] }))
    : l.words === '' ? [] : [{ key: 'w', node: l.words as ReactNode, cls: l.yours ? 'prw-acc' : 'prw-dim' }];
  const parts: { key: string; node: ReactNode; cls: string; tip?: string }[] = [
    ...(tag ? [{ key: 'tag', node: l.sigil, cls: 'prw-tag' }] : []),
    ...words,
    ...(l.late === '' ? [] : [{ key: 'late', node: l.late, cls: 'prw-late', tip: panel.pr_as_of_tip }]),
    ...(tail === undefined || tail === '' ? [] : [{ key: 'tail', node: tail, cls: 'prw-dim' }]),
  ];
  // in a row, his part goes with the tag as one head: the tag gives way first, so with little room
  // 'your merge' stays alone in the accent (designer m_54766)
  const [t0, w0] = parts;
  if (fit && tag && t0 !== undefined && w0 !== undefined && w0.cls === 'prw-acc') {
    return (
      <span className="prw prw-fit" data-testid={testId}>
        <span className="prw-head">
          <span className="prw-tag prw-shrink">{t0.node}{' · '}</span>
          <span className="prw-acc" data-acc="">{w0.node}</span>
        </span>
        {parts.slice(2).map((p) => (
          <span key={p.key} className={p.cls} {...(p.tip === undefined ? {} : tipAttrs(p.tip))}>
            {' · '}
            {p.node}
          </span>
        ))}
      </span>
    );
  }
  return (
    <span className={fit ? 'prw prw-fit' : 'prw'} data-testid={testId}>
      {parts.map((p, i) => (
        <span key={p.key} className={p.cls} data-acc={p.cls === 'prw-acc' ? '' : undefined} {...(p.tip === undefined ? {} : tipAttrs(p.tip))}>
          {i > 0 ? ' · ' : ''}
          {onFail !== undefined && p.cls === 'prw-fail' ? (
            <Press look="prw-press" onClick={onFail} data-testid={testId === undefined ? undefined : `${testId}.fail`}>{p.node}</Press>
          ) : p.node}
        </span>
      ))}
    </span>
  );
}
