// The sections drawn (sections.ts is the split; page artifacts-signal v3, designer m_60689): each
// group's head over the grid ('Kept · 4', 'Open work · 12'), the two quiet rows under For you
// ('Done · 199 ▸', 'Between agents · 1 234 ▸'), the back row over those lists ('← For you'), and
// KeptMark.tsx draws a kept card's ◆. Each is small and placed by the screen (and, on plugin-apps, by the app filter's gallery). Owner:
// artifacts-keep.
import { Press, tipAttrs } from '@/ds';
import { library } from '@/proto/words_library.gen';
import { topOf, type HeadOf } from './gallery';
import { sectionOf, type SectionId, type SectionView } from './sections';

/** A section's name. */
export function sectionWord(id: SectionId): string {
  return { kept: library.sect_kept, open: library.sect_open, done: library.sect_done, agents: library.sect_agents }[id];
}

/** The grid's group heads by section, with what each holds as shown: 'Kept · 4'. Pure. */
export function sectionHeads(n: Readonly<Record<SectionId, number>>): HeadOf {
  return (c) => {
    const id = sectionOf(topOf(c).a);
    return { key: id, title: `${sectionWord(id)} · ${n[id].toLocaleString()}` };
  };
}

/** Under For you: the two lists one click away, each with what it holds; none when empty. */
export function QuietRows({ n, onGo, testId }: { n: Readonly<Record<SectionId, number>>; onGo: (v: SectionView) => void; testId: string }) {
  const rows = (['done', 'agents'] as const).filter((id) => n[id] > 0);
  if (rows.length === 0) return null;
  return (
    <div className="art-quiet" data-testid={testId}>
      {rows.map((id) => (
        <Press key={id} look="art-quiet-row" onClick={() => { onGo(id); }} data-testid={`${testId}.${id}`}
          {...tipAttrs(id === 'done' ? library.sect_done_tip : library.sect_agents_tip)}>
          {`${sectionWord(id)} · ${n[id].toLocaleString()} ▸`}
        </Press>
      ))}
    </div>
  );
}

/** Over Done or Between agents: back to For you (⌘← does the same: a history step). */
export function BackRow({ view, n, onBack, testId }: { view: SectionView; n: number; onBack: () => void; testId: string }) {
  if (view === 'you') return null;
  return (
    <div className="art-back" data-testid={testId}>
      <Press look="art-back-row" onClick={onBack} data-testid={`${testId}.go`}>{library.back_for_you}</Press>
      <span className="art-back-n">{`${sectionWord(view)} · ${n.toLocaleString()}`}</span>
    </div>
  );
}
