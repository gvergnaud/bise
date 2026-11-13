// One card of the gallery (page artifacts-gallery v7, block b-card, every state): the picture, a
// name and one line (density rule 1, grid.ts lineOf). On hover or focus only, in place, nothing
// moving (rule 2): ··· (its menu), the pick circle, the extension at the picture's corner, a link's
// '↗ domain'. One hue, for what is new: the pink 'new' chip (rule 3); the cursor and a pick are a
// 2 px ink ring, several picked fill their circles. A stack: two sheets behind its cover and its
// count at the top right ('16 shots saved in one turn'). Every mark has its tip
// (page block tips). The picture inside the 16:10 box is art-thumbs' <Thumb>. Owner: art-grid.
import { memo, type HTMLAttributes, type MouseEvent, type ReactNode } from 'react';
import { iconKindOf, KindIcon, Press, tipAttrs } from '@/ds';
import { useRouter } from '@/app/router';
import { Thumb, thumbExt } from './thumbs';
import { countTip, domainOf, isNew, lineTip, titleOf, topOf, type Card } from './gallery';
import { opensTip } from './apps';
import { library } from '@/proto/words_library.gen';
import { panel } from '@/proto/words_panel.gen';
import { leaves, type Item } from './model';
import { KeptMark } from './KeptMark';

/** How a click picked: ⌘ adds or takes one, shift picks the run up to it. */
export interface PickHow {
  readonly meta: boolean;
  readonly shift: boolean;
}

/** A click on a card or a row: how it picks, and whether it opens (a plain click; ⌘ or ⇧ only pick). */
export function clickOf(e: { readonly metaKey: boolean; readonly ctrlKey: boolean; readonly shiftKey: boolean }): { how: PickHow; open: boolean } {
  const how = { meta: e.metaKey || e.ctrlKey, shift: e.shiftKey };
  return { how, open: !how.meta && !how.shift };
}

/** The click handler of a card or a row: it picks first (the cursor stays on it), then a plain click opens. */
export function onCardClick(key: string, onPick: (key: string, how: PickHow) => void, onOpen: () => void) {
  return (e: MouseEvent) => {
    const c = clickOf(e);
    onPick(key, c.how);
    if (c.open) onOpen();
  };
}

/** The picture of a card: the 16:10 box, the stack's sheets behind, the marks on its corners. */
export function CardPic({ card, it, visible, picked, onPick, onMenu, testId }: {
  card: Card | null;
  it: Item;
  visible: boolean;
  /** several picked: the circle filled */
  picked: boolean;
  onPick?: (how: PickHow) => void;
  onMenu?: (at: { x: number; y: number }) => void;
  testId: string;
}) {
  const ext = thumbExt(it);
  const out = leaves(it.a);
  const domain = domainOf(it.a);
  const stack = card?.kind === 'stack' ? card : null;
  // an expired file (artifacts_retention): no picture, its type icon in text-3 (designer m_62289)
  if (it.a.expired === true) {
    return (
      <span className="art-card-pic">
        <span className="art-card-th expired" data-testid={`${testId}.pic`} {...tipAttrs(library.expired_tip)}>
          <span className="art-card-xi"><KindIcon kind={iconKindOf(it.a)} size={24} /></span>
        </span>
      </span>
    );
  }
  return (
    <span className={`art-card-pic${stack ? ' stack' : ''}`}>
      {stack && <span className="art-card-s2" aria-hidden="true" />}
      {stack && <span className="art-card-s1" aria-hidden="true" />}
      <span className="art-card-th" data-testid={`${testId}.pic`}>
        <Thumb item={it} visible={visible} />
        {ext !== null && !out && <span className="art-card-x" data-testid={`${testId}.ext`} {...tipAttrs(opensTip(it.a) ?? undefined)}>{ext}</span>}
        {stack && <span className="art-card-n" data-testid={`${testId}.count`} {...tipAttrs(countTip(stack))}>{stack.items.length}</span>}
        {/* a PR hovers like any artifact: ⋯ and its tip, no 'github.com' over its tile (designer m_54748) */}
        {out && it.a.pr == null && <span className="art-card-lk" data-testid={`${testId}.out`} {...tipAttrs(library.tip_leaves)}>{domain === '' ? '↗' : `↗ ${domain}`}</span>}
      </span>
      {onPick && (
        <Press tabIndex={-1} look={`art-card-ck${picked ? ' y' : ''}`} aria-label="pick" aria-pressed={picked}
          data-testid={`${testId}.pick`} {...tipAttrs(library.tip_pick)}
          onClick={(e) => { e.stopPropagation(); onPick({ meta: true, shift: false }); }}>
          {picked ? '✓' : ''}
        </Press>
      )}
      {onMenu && (
        <Press tabIndex={-1} look="art-card-mb" aria-label="more" data-testid={`${testId}.menu`} {...tipAttrs(library.tip_more)}
          onClick={(e) => {
            e.stopPropagation();
            const r = e.currentTarget.getBoundingClientRect();
            onMenu({ x: r.left, y: r.bottom });
          }}>
          ···
        </Press>
      )}
    </span>
  );
}

/**
 * An expired card's line (designer m_62289): 'file expired · perf's thread ›', the link in text-2,
 * ink on hover; it opens the agent's thread and never the card.
 */
export function ExpiredLine({ it, testId }: { it: Item; testId: string }) {
  const router = useRouter();
  return (
    <>
      {library.file_expired} ·{' '}
      <Press look="art-card-thr" data-testid={`${testId}.thread`}
        onClick={(e) => { e.stopPropagation(); router.go({ screen: 'thread', project: it.project, agent: it.a.agent }); }}>
        {library.expired_thread({ agent: it.a.agent })}
      </Press>
    </>
  );
}

export interface CardProps {
  card: Card;
  /** the line under its name (grid.ts lineOf; a PR's: its words, PrWords) */
  line: ReactNode;
  /** the keyboard cursor: its ring */
  on: boolean;
  /** in his multi-pick */
  picked: boolean;
  /** several picked: the circles fill */
  many: boolean;
  /** in the virtualizer's range (art-thumbs draws these first) */
  visible: boolean;
  onPick: (key: string, how: PickHow) => void;
  onOpen: (card: Card) => void;
  onMenu?: (key: string, at: { x: number; y: number }) => void;
  dragOf?: (key: string) => HTMLAttributes<HTMLElement>;
  testId: string;
}

/** One card, every state of block b-card. */
export const ArtCard = memo(function ArtCard({ card, line, on, picked, many, visible, onPick, onOpen, onMenu, dragOf, testId }: CardProps) {
  const it = topOf(card);
  const name = titleOf(card);
  const menu = onMenu === undefined ? undefined : (at: { x: number; y: number }) => { onMenu(card.key, at); };
  return (
    <div {...dragOf?.(card.key)} className={`art-card${on ? ' on' : ''}${picked ? ' picked' : ''}`} role="option" aria-selected={on || picked}
      data-row={card.key} data-testid={testId} data-stack={card.kind === 'stack' || undefined}
      onClick={onCardClick(card.key, onPick, () => { onOpen(card); })}
      onContextMenu={menu === undefined ? undefined : (e) => { e.preventDefault(); menu({ x: e.clientX, y: e.clientY }); }}>
      <CardPic card={card} it={it} visible={visible} picked={many && picked}
        onPick={(how) => { onPick(card.key, how); }} {...(menu === undefined ? {} : { onMenu: menu })} testId={testId} />
      <span className="art-card-tt">
        <KeptMark a={it.a} testId={testId} />
        <span className={`art-card-t1${it.a.expired === true ? ' expired' : ''}`} {...tipAttrs(name)}>{name}</span>
        {isNew(card) && <span className="art-new" data-testid={`${testId}.new`} {...tipAttrs(library.tip_new)}>{panel.new_chip}</span>}
      </span>
      <span className="art-card-mt" data-testid={`${testId}.line`} {...tipAttrs(lineTip(card), undefined, 'below')}>{line}</span>
    </div>
  );
});
