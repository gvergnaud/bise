// src/app/side3d · the agents float with Overview's rows (page folded-side v9, Gabriel's pick: A's
// floats; designer m_45474): the same rows as the Overview's AGENTS (app/agentrows.ts): main
// first as 'bise', each agent its 40 head, then its mark (∿ at work, ? asks, ✗ failed, ✓ done, ⌥N
// while ⌥ is held), its name, the computer use mark when it uses his computer, its time, its
// context ring with its % from 70, its PR's number (a click opens the review); its now line under
// it, in red when it failed. An agent that asks him: its row on --c-accent-soft with the card's
// first two options in one click and 'answer…'. A worktree two agents share: its 'ψ <branch> · N
// agents' head with its PR, its agents under it; an open PR with no agent its own row. The foot
// folds ARCHIVED open ('▸ 3 archived'). Owner: nm-side.
import { useRef, type MouseEvent } from 'react';
import type { Agent, Card } from '@/proto';
import { AppIcon, Press, Ring, tipAttrs } from '@/ds';
import { sortCards } from '@/inbox/order';
import { useStore } from '@/store';
import type { AnswerAt } from '@/store/answered';
import { panel } from '@/proto/words_panel.gen';
import { side } from '@/proto/words_side.gen';
import { markTip } from '../tipwords';
import { AgentSign } from '../Marks';
import { askInbox } from '../inboxAsk';
import { useEntityMenu } from '../menus/use';
import { roleWords, type BarRow, type ProjAgent } from '../barrows';
import { ThingPic } from './Bubbles';
import { askButtons, pickIntent } from './model';
import { groupParts } from './lists';
import { OptButton } from './FlyoutAgents';
import { useCalm } from './useCalm';

const keyOfRow = (r: BarRow): string => r.key;

/** a row's test id in the float: the Overview's own, under the float's ('app.flyout.agents.agent.perf'), never the same id twice in the window */
export const floatId = (r: BarRow): string =>
  r.kind === 'agent' || (r.kind === undefined && r.agent !== undefined && !r.testId.startsWith('app.right.archived.'))
    ? `app.flyout.agents.${r.project}.${r.agent ?? ''}`
    : r.testId.replace(/^app\.right\./u, 'app.flyout.agents.');

/** a row's head, css px */
export const ROW_HEAD_PX = 32;

export interface FloatAgentsProps {
  readonly live: readonly BarRow[];
  readonly agents: readonly ProjAgent[];
  readonly cards: readonly Card[];
  /** where an answer is made from (the thread in view) */
  readonly at: AnswerAt | null;
  readonly onOpen: (r: BarRow) => void;
  /** a mark or a head's PR: the review (an orphan's: its branch) */
  readonly onOpenPr: (r: BarRow, e?: MouseEvent) => void;
  /** a row's project's name on bise's own screen ('acme › perf', designer m_45706); none in a project */
  readonly groupOf?: (project: string) => string | undefined;
  /** 'answer…' hands over to the composer: the float closes */
  readonly onHandoff: () => void;
}

/** the composer's field, where 'answer…' puts his words */
const COMPOSER_FIELD = '[data-testid="composer.field"]';

/** The agents float's rows. */
export function FloatAgents({ live, agents, cards, at, onOpen, onOpenPr, groupOf, onHandoff }: FloatAgentsProps) {
  const byKey = new Map(agents.map((x) => [`${x.project}/${x.a.name}`, x.a]));
  // the order holds under the pointer and slides when it moves (useCalm, designer m_47454)
  const box = useRef<HTMLDivElement>(null);
  const calm = useCalm(live, keyOfRow, box);
  return (
    <div ref={box} className="s3-arows" {...calm.bind} data-testid="app.flyout.agents.rows">
      {calm.rows.map((r) => {
        if (r.kind === 'group' || r.kind === 'orphan') return <HeadRow key={r.key} r={r} members={live.filter((m) => m.kind === 'agent' && m.group === r.group).length} onOpen={onOpen} onOpenPr={onOpenPr} />;
        const a = byKey.get(`${r.project}/${r.agent ?? ''}`);
        const card = sortCards(cards.filter((c) => c.agent === r.agent && c.project === r.project))[0];
        return <AgentRow key={r.key} r={r} a={a} card={card} at={at} onOpen={onOpen} onOpenPr={onOpenPr} group={groupOf?.(r.project)} onHandoff={onHandoff} />;
      })}
    </div>
  );
}

/** A row's PR: its number when it has one, else its worktree's mark; a click opens the review. */
function Pr({ r, onOpenPr }: { r: BarRow; onOpenPr: (r: BarRow, e?: MouseEvent) => void }) {
  const m = r.mark;
  if (m === undefined) return null;
  return (
    <span className="s3-arow-pr" data-tone={m.tone} role="button" aria-label={m.tip} {...tipAttrs(markTip(m, r.kind === 'orphan'))}
      onClick={(e) => {
        e.stopPropagation();
        onOpenPr(r, e);
      }} data-testid={`${floatId(r)}.pr`}>
      {m.pr !== undefined ? `#${String(m.pr)}` : m.g}
    </span>
  );
}

function AgentRow({ r, a, card, at, onOpen, onOpenPr, group, onHandoff }: { r: BarRow; a: Agent | undefined; card: Card | undefined; at: AnswerAt | null; onOpen: (r: BarRow) => void; onOpenPr: (r: BarRow, e?: MouseEvent) => void; group: string | undefined; onHandoff: () => void }) {
  const g = groupParts(group, r.name);
  const store = useStore();
  const menu = useEntityMenu({ kind: 'agent', project: r.project, agent: r.agent ?? '' });
  const asks = card !== undefined;
  const b = card === undefined ? null : askButtons(card);
  const failed = r.glyph === '✗';
  const id = floatId(r);
  const st = r.state;
  const job = asks ? card.question : st?.l2 !== undefined ? st.l2 : group !== undefined && a !== undefined ? roleWords(a.note || a.title || a.purpose) : r.sub;
  const who = `${r.name} · ${group !== undefined && a !== undefined ? roleWords(a.note || a.title || a.purpose) : r.sub}`;
  return (
    <div className="s3-arow" data-asks={asks || undefined} data-back={st?.back === true || undefined} data-group={r.group} data-calm={r.key} onContextMenu={menu.onContextMenu} data-testid={id}>
      <Press look="s3-arow-open" aria-label={r.title} onClick={() => {
        onOpen(r);
      }} data-row="">
        <span className="s3-head s3-arow-head" {...tipAttrs(who)}>
          <span className="s3-head-pic">
            <ThingPic thing={{ head: r.agent ?? '', main: a?.main === true }} px={ROW_HEAD_PX} tile="thread" />
          </span>
        </span>
        <span className="s3-arow-text">
          <span className="s3-arow-l1">
            {r.glyph.startsWith('⌥') && <span className="s3-arow-key" data-testid={`${id}.sign`}>{r.glyph}</span>}
            <span className="s3-arow-name" {...tipAttrs(who)} data-testid={`${id}.name`}>{g.project !== '' && <span className="s3-group">{g.project}</span>}{g.name}</span>
            {r.cu !== undefined && (
              <span className={`app-cu ${r.cu.tone}`} {...tipAttrs(r.cu.title)} data-testid={`${id}.computer`}>
                {r.cu.g}
              </span>
            )}
            {r.ctx !== undefined && r.pct !== undefined && r.pct !== '' && (
              <span className="s3-arow-pct" {...tipAttrs(r.tips?.pct)} data-testid={`${id}.pct`}>
                <Ring pct={r.ctx} testId={`${id}.ring`} />
                {r.pct}
              </span>
            )}
            <Pr r={r} onOpenPr={onOpenPr} />
          </span>
          <span className="s3-arow-l2" data-err={failed || undefined} data-testid={`${id}.role`}>{job}</span>
        </span>
        {st !== undefined && (
          <span className="s3-arow-state" data-tone={st.tone} {...tipAttrs(st.tip)} data-testid={`${id}.state`}>
            {/* the ask's mark has one form across the window: the circled ? (designer m_45861) */}
            {st.g === '∿' ? <AgentSign mark="works" testId={`${id}.state`} /> : st.g === '?' ? <AgentSign mark="asks" testId={`${id}.state`} /> : st.g !== '' && <span className="s3-arow-sg">{st.g}</span>}
            {st.words}
          </span>
        )}
      </Press>
      {b !== null && card !== undefined && (
        <span className="s3-arow-acts">
          {/* the options grow to 2 lines then clamp, the full words in the tip when cut (float-buttons dd382b7d3, his m_45871) */}
          {b.opts.map((o) => (
            <OptButton key={o.n} label={o.label} onPick={() => store.do(pickIntent(card, o.n, at))} testId={`${id}.opt.${String(o.n)}`} />
          ))}
          {(
            <Press look="s3-arow-answer" onClick={() => {
              // the float hands over to the composer (ui-review m_45821): it closes, the inbox opens on
              // that card in place, and his words go to the composer's field at once
              onHandoff();
              askInbox({ project: card.project, card: card.id });
              requestAnimationFrame(() => {
                document.querySelector<HTMLElement>(COMPOSER_FIELD)?.focus();
              });
            }} data-testid={`${id}.answer`}>
              {/* the word follows the card's kind (designer m_45824): a question 'answer…' (the composer
                  answers it), an approval 'write to perf…' (yes / no stay the only way to decide) */}
              {card.approval === true ? side.write_to({ agent: r.name }) : side.answer}
            </Press>
          )}
        </span>
      )}
    </div>
  );
}

/** A worktree's head ('ψ auth-v2 · 2 agents', its PR) or an open PR with no agent. */
function HeadRow({ r, members, onOpen, onOpenPr }: { r: BarRow; members: number; onOpen: (r: BarRow) => void; onOpenPr: (r: BarRow, e?: MouseEvent) => void }) {
  const id = floatId(r);
  return (
    <Press look="s3-arow-group" aria-label={r.title} {...tipAttrs(r.title)} onClick={() => {
      onOpen(r);
    }} data-row="" data-orphan={r.kind === 'orphan' || undefined} data-calm={r.key} data-testid={id}>
      <span className="s3-arow-g" aria-hidden="true">{r.kind === 'group' ? r.glyph : <AppIcon name="branch" size={14} />}</span>
      <span className="s3-arow-gname">{r.name}</span>
      {r.kind === 'group' && members > 0 && <span className="s3-arow-gn">{panel.tip_agents({ n: members })}</span>}
      <span className="s3-arow-cols">
        <Pr r={r} onOpenPr={onOpenPr} />
      </span>
    </Press>
  );
}

/** The foot's fold: '▸ 3 archived'; its rows open at the end of the float's rows (ArchivedRows), so the float grows by them. */
export function ArchivedFold({ n, open, onToggle }: { n: number; open: boolean; onToggle: () => void }) {
  if (n === 0) return null;
  return (
    <Press look="s3-fold" aria-expanded={open} onClick={onToggle} data-testid="app.flyout.agents.archived">
      <span className="s3-fold-tri" data-open={open || undefined} aria-hidden="true">▸</span>
      {side.archived({ n })}
    </Press>
  );
}

/** ARCHIVED's rows, open: their sign, name and age, newest first. */
export function ArchivedRows({ rows, onOpen }: { rows: readonly BarRow[]; onOpen: (r: BarRow) => void }) {
  return (
    <span className="s3-archived" data-testid="app.flyout.agents.archived.rows">
      {rows.map((r) => (
        <Press key={r.key} look="s3-arow-old" aria-label={r.title} {...tipAttrs(r.title)} onClick={() => {
          onOpen(r);
        }} data-row="" data-testid={floatId(r)}>
          <span className={`s3-arow-g ${r.tone}`} aria-hidden="true">{r.glyph}</span>
          <span className="s3-arow-gname">{r.name}</span>
          <span className="s3-arow-time">{r.end}</span>
        </Press>
      ))}
    </span>
  );
}
