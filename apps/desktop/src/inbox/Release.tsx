// The update card's body (update-dot, designer m_46058): a newer bise is ONE inbox card for the
// machine, and opened it says why it is safe to update now, then the release's changelog. Under
// the head ('bise v2026.10.3 is ready'): the safe line (t-13 text-2); the changelog as markdown
// (headings t-13/600 ink, items t-13 text-2, 4 apart), at most 240 high, then it scrolls inside the
// card with 16 px fades, the options staying in sight under it; last, t-12 text-3, 'you're on
// v2026.10.2 · full notes on GitHub ↗' (the link opens in his browser). No notes: 'no notes for
// this release.'. The hub carries it all on the card (Card.release, Card.link), never parsed from
// its text. Owner: amb-home.
import { useLayoutEffect, useRef, useState } from 'react';
import type { Card } from '@/proto';
import { settings } from '@/proto/words_settings.gen';
import { Markdown } from '@/thread/Markdown';

const open = (url: string): void => {
  void (window as { bise?: { openExternal?: (u: string) => Promise<boolean> } }).bise?.openExternal?.(url);
};

/** Whether the box scrolls, and whether he is past its top and before its end (the fades; the
 *  settled card's middle too, InboxCard). */
export function useFades(box: React.RefObject<HTMLDivElement | null>): { top: boolean; end: boolean } {
  const [f, setF] = useState({ top: false, end: false });
  useLayoutEffect(() => {
    const el = box.current;
    if (el === null) return undefined;
    const read = () => {
      const top = el.scrollTop > 0;
      const end = el.scrollTop + el.clientHeight < el.scrollHeight - 1;
      setF((was) => (was.top === top && was.end === end ? was : { top, end }));
    };
    read();
    el.addEventListener('scroll', read, { passive: true });
    return () => {
      el.removeEventListener('scroll', read);
    };
  }, [box]);
  return f;
}

/** The update card's release under its head; nothing for any other card. */
export function ReleaseBody({ card, id }: { card: Pick<Card, 'kind' | 'release' | 'link'>; id: string }) {
  const box = useRef<HTMLDivElement>(null);
  const fades = useFades(box);
  const r = card.release;
  if (card.kind !== 'update' || r == null) return null;
  const notes = r.notes.trim();
  const link = card.link ?? null;
  return (
    <div className="ic-rel" data-testid={`${id}.release`}>
      <p className="ic-rel-safe" data-testid={`${id}.release.safe`}>
        {settings.bise_update_tip}
      </p>
      {notes === '' ? (
        <p className="ic-rel-none" data-testid={`${id}.release.none`}>
          {settings.bise_update_no_notes}
        </p>
      ) : (
        <div ref={box} className="ic-rel-notes" data-top={fades.top || undefined} data-end={fades.end || undefined} data-testid={`${id}.release.notes`}>
          <Markdown text={notes} onLink={open} />
        </div>
      )}
      <p className="ic-rel-foot" data-testid={`${id}.release.foot`}>
        {settings.bise_update_on({ running: r.running })}
        {link !== null && (
          <>
            {' · '}
            <a
              href={link}
              onClick={(e) => {
                e.preventDefault();
                open(link);
              }}
              data-testid={`${id}.release.link`}
            >
              {settings.bise_update_full_notes}
            </a>
          </>
        )}
      </p>
    </div>
  );
}
