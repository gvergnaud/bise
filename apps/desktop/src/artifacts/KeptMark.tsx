// The ◆ of a kept card (page artifacts-signal v3, designer m_60689): ink when he kept it, text-3
// when an agent did (its line says which, gallery.ts lineOf), its tip naming who and how to let it
// go. One small component the card places (and, on plugin-apps, the app gallery's card). Owner:
// artifacts-keep.
import { tipAttrs } from '@/ds';
import type { Artifact } from '@/proto';
import { library } from '@/proto/words_library.gen';
import { keptByHim } from './sections';

/** The ◆ of a kept card, with its tip; nothing when it is not kept. */
export function KeptMark({ a, testId }: { a: Pick<Artifact, 'kept'>; testId: string }) {
  if (a.kept == null) return null;
  const his = keptByHim(a);
  return (
    <span className={`art-kept${his ? '' : ' agent'}`} data-testid={`${testId}.kept`} data-by={a.kept.by}
      {...tipAttrs(his ? library.kept_tip_you : library.kept_tip_agent({ agent: a.kept.by }))}>◆</span>
  );
}
