// One feature (bar A.6, amb-feed 5f436473; bise fait mac §9: its own head, no banner): its branch and
// base, how far it is from the trunk, its state (checked, a try building, on trial), try / merge /
// drop through its card (no typed command: store.answer with the card's id, architect m_10203; the
// merge card's drop asks once more, as the TUI does), then its agents as ds ListRows under a ds
// GroupLabel, each with its menu. The floating composer's room at the foot. Owner: nm-pages-b
// (amb-win's before).
import { useEffect, useRef, useState } from 'react';
import { usePageColumn } from '@/app/pagecol';
import { Button, Empty, GroupLabel, Head, ListRow, MoreButton } from '@/ds';
import type { Agent } from '@/proto';
import { pages } from '@/proto/words_pages.gen';
import { useEntityMenu } from '@/app/menus/use';
import { asksHim, StateMark } from '@/agents/AgentsScreen';
import { useAgents, useCards, useFeatures, useStore } from '@/store';
import { useRouter } from '@/app/router';
import { useKeptScroll } from '@/app/keep';
import { endOf } from '@/agents/model';
import { useNow } from '@/app/clock';
import { cardOf, choicesOf, distance, dropAsk, stateWords } from './feature';
import './review.css';

export function FeatureScreen({ project, name }: { project: string; name: string }) {
  useKeptScroll(`feature:${project}/${name}`, () => document.querySelector<HTMLElement>('[data-testid="feature.screen"] .rv-body'));
  const store = useStore();
  const features = useFeatures(project);
  const cards = useCards(project);
  const agents = useAgents(project);
  const now = useNow();
  const [asking, setAsking] = useState(false);
  const screen = useRef<HTMLElement>(null);
  const col = usePageColumn(screen);

  // a fresh list on open
  useEffect(() => {
    store.command({ cmd: 'features', project });
  }, [store, project]);

  const f = features.find((x) => x.name === name);
  if (f === undefined) {
    return (
      <section ref={screen} style={col} className="rv-screen" data-testid="feature.screen" data-project={project} data-feature={name}>
        <Head title={name} sub={pages.feature_word} />
        <div className="rv-body">
          <Empty testId="feature.none" title={pages.feature_none}>{pages.feature_none_why}</Empty>
        </div>
      </section>
    );
  }
  const card = cardOf(f, cards);
  const choices = card ? choicesOf(card, asking) : [];
  const mine = agents.filter((a) => f.agents.includes(a.name));
  return (
    <section ref={screen} style={col} className="rv-screen" data-testid="feature.screen" data-project={project} data-feature={name}>
      <Head title={f.name} sub={`${f.branch} → ${f.base} · ${distance(f)}`} />
      <div className="rv-body">
        <p className="rv-note" data-testid="feature.state" data-building={f.building} data-trial={f.trial} data-checked={f.checked}>
          {stateWords(f)}
          {f.adopted === true ? ` · ${pages.feature_existing}` : ''}
        </p>
        {f.tried != null && (
          <p className="rv-note" data-testid="feature.tried">
            {pages.feature_tried({ sha: f.tried.sha })} <code>{f.tried.run}</code>
          </p>
        )}

        {card !== undefined && (
          <div className="ft-card" data-testid={`feature.card.${String(card.id)}`} data-kind={card.kind}>
            <p className="ft-q">{asking ? dropAsk(f) : card.question}</p>
            <div className="ft-acts">
              {choices.map((c, i) => (
                <Button
                  key={c.label}
                  kind={i === 0 ? 'pri' : c.confirm || (asking && c.reply !== undefined) ? 'danger' : 'ghost'}
                  testId={c.back ? 'feature.keep' : c.confirm ? 'feature.drop' : asking ? 'feature.drop.yes' : `feature.opt.${c.reply ?? String(c.n ?? '')}`}
                  onClick={() => {
                    if (c.act !== undefined) store.do({ i: 'open', project, act: c.act });
                    else if (c.confirm) setAsking(true);
                    else if (c.back) setAsking(false);
                    else if (c.reply !== undefined) {
                      setAsking(false);
                      store.do({ i: 'answer', project, card: card.id, reply: c.reply });
                    }
                  }}
                >
                  {c.label}
                </Button>
              ))}
            </div>
          </div>
        )}
        {card === undefined && f.card == null && (
          <p className="rv-note" data-testid="feature.nocard">
            {f.trial ? pages.feature_on_trial : pages.feature_try_when}
          </p>
        )}

        <GroupLabel count={mine.length} testId="feature.agents">{pages.feature_agents}</GroupLabel>
        {mine.length === 0 && <Empty testId="feature.noagent" title={pages.feature_no_agent} />}
        {mine.map((a) => <AgentRow key={a.name} project={project} a={a} now={now} />)}
      </div>
    </section>
  );
}

/** One agent of the feature (ds ListRow): its state mark, its name, its end; its '…' and a right click are its menu. */
function AgentRow({ project, a, now }: { project: string; a: Agent; now: number }) {
  const router = useRouter();
  const menu = useEntityMenu({ kind: 'agent', project, agent: a.name });
  const end = endOf(a, now);
  const tid = `feature.agent.${a.name}`;
  return (
    <ListRow testId={tid} state={<StateMark a={a} />} title={a.name}
      sub={<span className={asksHim(a) ? 'ag-end wait' : 'ag-end'}>{end.text}</span>}
      onContextMenu={menu.onContextMenu}
      end={<MoreButton menu={menu} testId={`${tid}.more`} />}
      onClick={() => {
        router.go({ screen: 'thread', project, agent: a.name });
      }} />
  );
}
