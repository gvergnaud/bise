// A question of a task's, answered in place: its words, the inbox's keys (1-9) on its options, then
// his own words in the ds Field (tasks-ux m_64959, designer m_65805). The Task tab's Needs you and a
// Board card's '1 question ▾' draw it (designer m_65852). Owner: tasks.
import type { Card } from '@/proto';
import { Field, Kbd, Press } from '@/ds';
import { useStore } from '@/store';
import { inbox } from '@/proto/words_inbox.gen';
import { tasks } from '@/proto/words_tasks.gen';
import { askButtons, pickIntent } from '@/app/side3d/model';
import { firstLine } from './model';
import { CardMoveMore } from './MoveMenus';

export function Question({ c, project, testId }: { c: Card; project: string; testId: string }) {
  const store = useStore();
  const opts = askButtons(c).opts;
  return (
    <div className="tk-q" data-testid={testId}>
      <div className="tk-q-head">
        <p className="tk-q-text">{c.question}</p>
        {/* Move to task… (F9): a card belongs to one task, this one */}
        <CardMoveMore project={project} card={c.id} title={firstLine(c.question)} testId={`${testId}.more`} />
      </div>
      <ol className="tk-q-opts">
        {opts.map((o) => (
          <li key={o.n}>
            <Press look="tk-q-opt" onClick={() => store.do(pickIntent(c, o.n, null))} data-testid={`${testId}.${String(o.n)}`}>
              <Kbd>{String(o.n)}</Kbd>
              <span className="tk-q-opt-w">{o.label}</span>
            </Press>
          </li>
        ))}
      </ol>
      <Field placeholder={opts.length > 0 ? inbox.hint_or_answer : tasks.q_type} aria-label={c.question} testId={`${testId}.type`}
        onKeyDown={(e) => {
          const words = e.currentTarget.value.trim();
          if (e.key === 'Enter' && words !== '') store.do({ i: 'answer', project: c.project, card: c.id, reply: words });
          else if (e.key >= '1' && e.key <= '9' && e.currentTarget.value === '' && opts.some((o) => String(o.n) === e.key)) {
            e.preventDefault();
            store.do(pickIntent(c, Number(e.key), null));
          }
        }} />
    </div>
  );
}
