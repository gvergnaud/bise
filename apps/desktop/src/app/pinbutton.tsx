// The hover pin (his m_49100, native-lead m_49106): a row's own ds glyph button that takes its end's
// place while the pointer is on the row (or the button has the keyboard's focus); the whole row
// stays the open target. One sign for the state on every kind (artifact-pin, designer m_49505; his
// m_49499: « je ne sais pas si c'est pin ou pas pin »): pinned is the filled pin ('pin-on'), the
// outline only the hover offer. On hover: unpinned the outline in text-3, tip 'Pin'; pinned the
// filled one in text, tip 'Unpin'; no pressed square. A pinned row shows the filled mark at rest
// (PinMark, text-2, 12 in a 16 box), the pages too; an unpinned row shows nothing at rest. Its
// click pins or unpins through pins.ts, never opens the row, and gives feedback at once: the glyph
// and the tip flip before the hub's echo, with a 150 ms pop (none under reduced motion); a refused
// prefs_set flips it back, the tip 'Couldn't pin'. Its list keeps its order under his pointer
// (pinhold.ts). The rows that carry it place it (side3d.css .s3-pin, the pages' own CSS). Owner: nm-frame.
import { useEffect, useState, type MouseEvent } from 'react';
import { AppIcon, IconButton, tipAttrs } from '@/ds';
import { useStore } from '@/store';
import { menus } from '@/proto/words_menus.gen';
import { frame } from '@/proto/words_frame.gen';
import { pinThing, type PinKind } from './pins';
import { holdPin, scopeOf, unholdPin } from './pinhold';

/** What a row needs to carry its pin: its thing, and whether it is pinned now. */
export interface PinOf {
  readonly kind: PinKind;
  readonly project: string;
  /** its pin id (pins.ts: a name, an artifact id, a timer id, procPinId) */
  readonly id: string;
  readonly on: boolean;
}

/** His click, until the hub's echo: the state he asked for, and the state it came from. */
interface Want {
  readonly on: boolean;
  readonly was: boolean;
}

/** The state a pin shows: his click's while the prefs still say what it came from, else the prefs'. Pure. */
export const shownOn = (on: boolean, want: Want | null): boolean => (want !== null && on === want.was ? want.on : on);

/** The hover pin's tip: a refused click's words (the state he asked for), else what a click does. Pure. */
export const pinTip = (on: boolean, failed: boolean | null): string =>
  failed === null ? (on ? menus.unpin : menus.pin) : failed ? menus.pin_failed : menus.unpin_failed;

/** The hover pin: pins or unpins `pin`'s thing (module header). */
export function PinButton({ pin, look, size = 'row', hold = true, testId }: {
  pin: PinOf;
  look?: string;
  /** 'row' in a list row (16 wide; a tab's, panel/TabPin.tsx, 20 like its ×), 'bar' in a bar (24) */
  size?: 'row' | 'bar';
  /** a click holds its list's order under the pointer (pinhold.ts); a pin outside the lists holds nothing */
  hold?: boolean;
  testId: string;
}) {
  const store = useStore();
  const [want, setWant] = useState<Want | null>(null);
  /** a click the hub refused: the state it asked for (its tip says so until the pointer leaves) */
  const [failed, setFailed] = useState<boolean | null>(null);
  const [pops, setPops] = useState(0);
  const on = shownOn(pin.on, want);
  const waiting = want !== null && pin.on === want.was;
  const { kind, project, id } = pin;
  // a refused prefs_set while his click waits for its echo: back as it was
  useEffect(() => {
    if (!waiting) return undefined;
    const asked = want.on;
    return store.onEvent((ev) => {
      if (ev.ev !== 'error' || ev.cmd !== 'prefs_set') return;
      setWant(null);
      setFailed(asked);
      unholdPin(kind, project, id);
    });
  }, [store, waiting, want, kind, project, id]);
  const click = (e: MouseEvent<HTMLButtonElement>) => {
    e.stopPropagation();
    const next = !on;
    setWant({ on: next, was: pin.on });
    setFailed(null);
    setPops((n) => n + 1);
    // a click with the pointer holds its list's order until the pointer leaves the card (pinhold.ts)
    if (hold && e.detail > 0) holdPin({ kind, project, id, was: on }, scopeOf(e.currentTarget));
    pinThing(kind, project, id, next, store);
  };
  return (
    <span className={look ?? 's3-pin'} data-on={on || undefined} data-pop={pops > 0 || undefined} data-pin-key={`${kind}:${project}:${id}`}
      onPointerLeave={() => {
        setFailed(null);
      }}>
      <IconButton size={size} label={pinTip(on, failed)} pressed={on} onClick={click} testId={testId}>
        <AppIcon key={pops} name={on ? 'pin-on' : 'pin'} size={14} />
      </IconButton>
    </span>
  );
}

/** A pinned row's mark at rest: the filled pin, text-2, 12 in a 16 box, its tip 'Pinned'. */
export function PinMark({ testId }: { testId: string }) {
  return (
    <span className="b-pinmark" aria-label={frame.pinned} {...tipAttrs(frame.pinned)} data-testid={testId}>
      <AppIcon name="pin-on" size={14} />
    </span>
  );
}
