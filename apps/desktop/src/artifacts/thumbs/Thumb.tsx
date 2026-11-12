// An artifact card's picture (artifacts-gallery v7, blocks `kinds` and `b-card`): it fills the
// card's 16:10 frame (art-grid's Card draws the frame, its radius and the marks over it). First
// the kind's faint line icon, then the picture fades in over 160 ms; no shimmer, no spinner. A PR,
// a release, a link: a tile of words, no fetch. A file: Quick Look's picture (a md or a txt too);
// one it can't draw: its extension, big, and its size. A video: '▶ 0:04' bottom right. data-thumb
// says which: icon | img | tile | none.
// Owner: art-thumbs.
import { memo, useEffect, useMemo, useState } from 'react';
import { iconKindOf, KindIcon, PlayIcon } from '@/ds';
import type { ThumbAnswer } from '@/shared/api';
import type { Item } from '../model';
import { planOf, type Made, type Tile } from './plan';
import { reqKey } from './queue';
import { thumbQueue } from './run';
import { bytesWords, secsWords } from './words';
import './thumbs.css';

export interface ThumbProps {
  readonly item: Item;
  /** the card is in the grid's view (asked first); false: mounted just out of it */
  readonly visible: boolean;
}

/** The kind's faint icon: the loading state, and what stays when there is nothing to draw. */
function Faint({ item }: { item: Item }) {
  return (
    <span className="th-faint" aria-hidden="true">
      <KindIcon kind={iconKindOf(item.a)} size={16} />
    </span>
  );
}

function TileView({ t, item }: { t: Tile; item: Item }) {
  return (
    <span className="th th-tile" data-thumb="tile">
      <KindIcon kind={t.icon === 'link' ? iconKindOf(item.a) : t.icon} size={24} />
      <span className="th-w">{t.words}</span>
    </span>
  );
}

/** A made picture's answer drawn: the image (faded in), the no-preview or folder tile. */
function Answer({ a, item, fade }: { a: ThumbAnswer; item: Item; fade: boolean }) {
  const [shown, setShown] = useState(!fade);
  const secs = 'secs' in a && a.secs !== undefined ? secsWords(a.secs) : null;
  const play =
    secs === null ? null : (
      <span className="th-play">
        <PlayIcon />
        {secs}
      </span>
    );
  if ('img' in a) {
    return (
      <span className="th th-img" data-thumb="img">
        {!shown && <Faint item={item} />}
        <img src={a.img} alt="" draggable={false} className={shown ? 'in' : undefined} onLoad={() => {
            setShown(true);
          }}
        />
        {play}
      </span>
    );
  }
  if ('files' in a) {
    return (
      <span className="th th-tile" data-thumb="none">
        <KindIcon kind="file" size={24} />
        <span className="th-w">{`${String(a.files)} ${a.files === 1 ? 'file' : 'files'}`}</span>
      </span>
    );
  }
  // a sound: its line icon and its length (the extension is the card's, on hover)
  if (secs !== null && iconKindOf(item.a) === 'audio') {
    return (
      <span className="th th-tile" data-thumb="none">
        <KindIcon kind="audio" size={24} />
        <span className="th-w">{secs}</span>
      </span>
    );
  }
  return (
    <span className="th th-tile" data-thumb="none">
      <span className="th-ext">{a.none.ext === '' ? '?' : a.none.ext}</span>
      <span className="th-w">{bytesWords(a.none.bytes)}</span>
      {play}
    </span>
  );
}

/** A picture main makes (a file or a page): asked through the queue, in view first (keyed by its request: a new version starts over). */
function MadeView({ m, item, visible }: { m: Made; item: Item; visible: boolean }) {
  const q = thumbQueue();
  const req = useMemo(() => ({ how: m.how, src: m.src, project: m.project, id: m.id, v: m.v, sig: m.sig }), [m.how, m.src, m.project, m.id, m.v, m.sig]);
  const now = q?.known(reqKey(req));
  const [got, setGot] = useState<{ a: ThumbAnswer | null; fade: boolean } | null>(now === undefined ? null : { a: now, fade: false });
  useEffect(() => {
    if (q === null || got !== null) return;
    let live = true;
    void q.get(req, visible ? 'view' : 'near').then((a) => {
      if (live) setGot({ a, fade: true });
    });
    return () => {
      live = false;
      q.lower(req);
    };
  }, [q, req, visible, got]);
  if (got?.a == null) {
    return (
      <span className="th th-icon" data-thumb="icon">
        <Faint item={item} />
      </span>
    );
  }
  return <Answer a={got.a} item={item} fade={got.fade} />;
}

/** An artifact's picture, filling its parent (the card's 16:10 frame). */
export const Thumb = memo(function Thumb({ item, visible }: ThumbProps) {
  const plan = useMemo(() => planOf(item), [item]);
  if (plan.how === 'tile') return <TileView t={plan} item={item} />;
  if (plan.how === 'none') {
    return (
      <span className="th th-icon" data-thumb="icon">
        <Faint item={item} />
      </span>
    );
  }
  return <MadeView key={`${plan.project}/${plan.id}-v${String(plan.v)}.${plan.sig}`} m={plan} item={item} visible={visible} />;
});
