// The window's one status pill in the title bar's own row (designer's desktop-notices page v2,
// m_21067; ambient-lead m_21071): what bise does by itself (core down or restarted, an update
// ready, /update's build, a reload) says so in ONE pill right-aligned in the centre's title bar,
// 8 px before the right bar's toggle; nothing moves under it. The worst wins (status.ts); each
// candidate keeps its own words, actions and test ids (CoreLine.tsx, UpdateLine.tsx: amb-win's;
// ReloadLine.tsx). The pill takes at most the room between the crumb's end +16 and the toggle
// (amb-win's slot, app.title.status): its words cut with …, a click opens a 320 px popover with
// the whole words, the actions and ✕; under 160 px of room (the slot's data-narrow) only its sign
// and short words show. Owner: amb-home.
import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from 'react';
import { Act, IconButton, Press, cx, tipAttrs } from '@/ds';
import { Breath } from '@/working/Signs';
import { useCoreLine } from './CoreLine';
import { useReloadLine } from './ReloadLine';
import { useBiseUpdateLine, useHubUpdateLine, useUpdateLine } from './UpdateLine';
import { worst, type StatusPart } from './status';
import './status.css';

/** The status to show: every candidate's hook runs on every render (hooks' order), the worst wins. */
export function useStatus(): StatusPart | null {
  const core = useCoreLine();
  const reload = useReloadLine();
  const update = useUpdateLine();
  const hubUpdate = useHubUpdateLine();
  const biseUpdate = useBiseUpdateLine();
  // the order breaks a tie (designer: went_bad > core down > failed > update > reload > done); a newer bise
  // for the machine says the app's own update too (its click installs both), so it goes first
  return worst([core, hubUpdate, biseUpdate, update, reload]);
}

/** The pill amb-win's title-bar slot draws; nothing while bise does nothing by itself. */
export function TitleStatus() {
  const part = useStatus();
  // a new candidate starts closed: its popover never carries over from another one
  return part === null ? null : <Pill key={part.testId} part={part} />;
}

/** The pill's words as one line for a tip: its whole words when they are text, else its short ones. Pure. */
export const tipWords = (part: Pick<StatusPart, 'words' | 'short'>): string => (typeof part.words === 'string' ? part.words : part.short);

/** When even the short words don't fit, the pill is its sign alone (the breath while running; designer m_21244). */
const GLYPH = { done: '✓', quiet: '↑', err: '▲' } as const;

/**
 * Whether even the short words don't fit the slot's room (designer m_21244): then the pill is a
 * 24 px square with its sign alone, and a click opens the popover. The short words stay laid out
 * (out of the flow, hidden) so their width is still known when the pill is tight.
 */
function useTight(box: RefObject<HTMLDivElement | null>, short: string): boolean {
  const [tight, setTight] = useState(false);
  useLayoutEffect(() => {
    const pill = box.current;
    const slot = pill?.parentElement;
    if (pill === null || slot === null || slot === undefined) return;
    const read = () => {
      const words = pill.querySelector<HTMLElement>('.app-ts-short');
      if (slot.dataset['narrow'] !== 'true' || words === null) {
        setTight(false);
        return;
      }
      const cs = getComputedStyle(pill);
      const breath = pill.querySelector<HTMLElement>('.breath');
      const lead = breath === null ? 0 : breath.offsetWidth + parseFloat(cs.columnGap || '0');
      const need = words.offsetWidth + lead + parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight);
      // the coins' minis share the slot (folded-side v9): the pill's room is what they leave
      const minis = slot.querySelector<HTMLElement>('.s3-minis-slot');
      const minisW = minis === null || minis.childElementCount === 0 ? 0 : minis.offsetWidth + parseFloat(getComputedStyle(minis).marginLeft || '0');
      setTight(slot.clientWidth - minisW < need);
    };
    read();
    const ro = new ResizeObserver(read);
    ro.observe(slot);
    // the slot's data-narrow flips after its own resize: read again then
    const mo = new MutationObserver(read);
    mo.observe(slot, { attributes: true, attributeFilter: ['data-narrow'] });
    return () => {
      ro.disconnect();
      mo.disconnect();
    };
  }, [box, short]);
  return tight;
}

function Pill({ part }: { part: StatusPart }) {
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  // the popover closes on a click outside it or esc (esc never closes the pill itself)
  useEffect(() => {
    if (!open) return;
    const away = (e: MouseEvent) => {
      if (!(e.target instanceof Node) || box.current?.contains(e.target) !== true) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    window.addEventListener('mousedown', away, true);
    window.addEventListener('keydown', esc, true);
    return () => {
      window.removeEventListener('mousedown', away, true);
      window.removeEventListener('keydown', esc, true);
    };
  }, [open]);
  const tight = useTight(box, part.short);
  const id = part.testId;
  // a calm part's failure keeps the neutral pill: only its ▲ is red (designer m_45423)
  const calm = part.calm === true;
  const err = part.tone === 'err' && !calm;
  const acts = part.acts ?? [];
  const actions = (pop: boolean) => (
    <>
      {acts.map((a) => (
        <Act key={a.id} tone={err ? 'err' : undefined} disabled={a.disabled} onClick={a.onClick} testId={pop ? `${id}.pop.${a.id}` : `${id}.${a.id}`}>
          {a.text}
        </Act>
      ))}
      {part.close !== undefined && (
        <IconButton label="close" size="row" onClick={part.close} testId={pop ? `${id}.pop.close` : `${id}.close`}>
          ✕
        </IconButton>
      )}
    </>
  );
  return (
    <div
      ref={box}
      className={cx('app-ts', calm && part.tone === 'err' ? 'quiet' : part.tone, calm && 'calm', part.fold === true && 'fold')}
      role={err ? 'alert' : 'status'}
      data-testid={id}
      data-state={part.state}
      data-tight={tight}
    >
      <Press
        look="app-ts-open"
        // its sign alone (tight): the tip and VoiceOver say what it is (a compact mark explains itself)
        {...(tight ? { 'aria-label': tipWords(part), ...tipAttrs(tipWords(part)) } : {})}
        aria-expanded={open}
        onClick={() => {
          setOpen((o) => !o);
        }}
        data-testid={`${id}.open`}
      >
        {part.tone === 'running' ? (
          <Breath testId={`${id}.breath`} />
        ) : (
          <span className="app-ts-glyph" aria-hidden="true" data-testid={`${id}.glyph`}>
            {GLYPH[part.tone]}
          </span>
        )}
        <span className="app-ts-words" data-testid={`${id}.text`}>
          {part.words}
        </span>
        <span className="app-ts-short" data-testid={`${id}.short`}>
          {part.short}
        </span>
        {part.age !== undefined && (
          <span className="app-ts-age" data-testid={`${id}.age`}>
            {part.age}
          </span>
        )}
      </Press>
      <span className="app-ts-acts">{actions(false)}</span>
      {open && (
        <div className="app-ts-pop" role="dialog" aria-label="status" data-testid={`${id}.pop`}>
          <div className="app-ts-pop-words">
            {part.words}
            {part.age !== undefined && <span className="app-ts-age">{` · ${part.age}`}</span>}
          </div>
          {part.more}
          {(acts.length > 0 || part.close !== undefined) && <div className="app-ts-pop-acts">{actions(true)}</div>}
        </div>
      )}
    </div>
  );
}
