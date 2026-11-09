// The capsule's card (a waiting row the spotlight opened): his gestures on it, and the capsule's
// edge for the store's effects. The capsule has no router and no panel: what the machine opens
// (store/machine/cards.ts opens) goes to the main window, in front, as an ask (app/route.ts
// artifactAsk: the artifact in its panel's tab at its block, the window stays where it is); a
// window.bise call runs here. The card stays waiting: the chip and an option that opens answer
// nothing (his m_53914: the chip did nothing, the capsule ran no effect). Owner: amb-mac.
import type { Card } from '@/proto';
import type { InboxCardActs } from '@/inbox/InboxCard';
import { chipOpen } from '@/inbox/about';
import { artifactAsk, cardRoute } from '@/shared/asks';
import { formatRoute } from '@/app/route';
import { navRoute } from '@/app/edge';
import type { Effect, FxName, Intent } from '@/store/machine/types';
import type { BiseApi } from '@/shared/api';

/** What the capsule's card reaches: the main window and the window.bise calls the machine names. */
export type CapsuleBise = Pick<BiseApi, 'openWindow' | FxName>;

/** The capsule's own moves for its card: later (esc), done (⌘⌫), close (it went to the window). */
export interface CapsuleMoves {
  readonly later: () => void;
  readonly done: () => void;
  readonly close: () => void;
}

/**
 * The card's acts as the capsule draws it (InboxCard): an option goes through the machine's pick
 * (an option that opens an artifact opens it, nothing sent; any other answers), the chip opens
 * what it is about, ⏎ opens this card in the window's inbox.
 */
export function capsuleCardActs(store: { do: (i: Intent) => unknown }, c: Card, bise: CapsuleBise | undefined, moves: CapsuleMoves): InboxCardActs {
  const chip = chipOpen(c);
  return {
    later: moves.later,
    close: moves.done,
    pick: (n) => {
      store.do({ i: 'pick', project: c.project, card: c.id, n, tab: true });
    },
    open: () => {
      bise?.openWindow(cardRoute(c.project, c.id));
      moves.close();
    },
    ...(chip === null
      ? {}
      : {
          openPage: () => {
            store.do(chip);
          },
        }),
  };
}

/** Runs one effect of the capsule's store: a move as the main window's ask, a call on `bise`. */
export function runCapsuleEffect(e: Effect, bise: CapsuleBise | undefined): void {
  switch (e.out) {
    case 'nav':
      // a branch's review: the main window on its screen; an artifact: its panel's tab
      bise?.openWindow('branch' in e.to ? formatRoute(navRoute(e.to)) : artifactAsk(e.to));
      return;
    case 'fx':
      void bise?.[e.fx.fx](...e.fx.args);
      return;
  }
}
