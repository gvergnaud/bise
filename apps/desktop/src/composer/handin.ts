// His words handed to a composer from outside it (the spotlight's ⏎ on an agent's row under a
// message, shared/asks.ts draftAsk): never sent, never lost. The composer of that target mounted
// takes them (putBack: into its field when empty, else on top of its history); none mounted: they
// are its saved draft, which the composer loads when it comes (a draft there already goes to its
// history first). Owner: the composer.
import { useEffect, type RefObject } from 'react';
import type { ComposerActions } from './actions';
import { load, remember, save, type Store } from './drafts';

const EVENT = 'bise:hand-in';

interface HandIn {
  readonly target: string;
  readonly entry: string;
}

/** His entry to the composer of `target`: the mounted one takes it, else its saved draft. */
export function handIn(store: Store, target: string, entry: string): void {
  const ev = new CustomEvent<HandIn>(EVENT, { detail: { target, entry }, cancelable: true });
  if (!window.dispatchEvent(ev)) return;
  const was = load(store, target);
  const kept = was.draft === null ? was : remember(was, was.draft);
  save(store, target, { ...kept, draft: entry });
}

/** The composer of `target` takes the entries handed to it while mounted. */
export function useHandIn(target: string, actions: RefObject<ComposerActions | null>): void {
  useEffect(() => {
    const take = (e: Event): void => {
      const d = (e as CustomEvent<HandIn>).detail;
      const a = actions.current;
      if (d.target !== target || a === null) return;
      e.preventDefault();
      a.putBack(d.entry);
      a.focusEnd();
    };
    window.addEventListener(EVENT, take);
    return () => {
      window.removeEventListener(EVENT, take);
    };
  }, [target, actions]);
}
