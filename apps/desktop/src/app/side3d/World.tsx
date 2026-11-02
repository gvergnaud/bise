// src/app/side3d · the bims' world floating at the window's top right (page native-mac §7,
// mockups 11 and 14): the project's office (bims/Office, `transparent`: the island alone, no frame,
// no ground) with a 3-layer drop shadow on its alpha (--sh-world on the wrapper: the filter follows
// the scaled canvas). Drawn once at 384 and shown at 148 by a scale from its top-right corner, so the
// hover's growth is one transform: after HOVER_MS of hover it grows to 384 in --d-base, a glass
// legend pill 8 under it (the project, '3 at their desks · 1 asking', ↗), floating over the
// bubbles, which never move (designer m_42536: a target never moves away from the pointer); it
// folds back GRACE_MS after the pointer leaves; never under reduced motion. A click opens the office in a tab. The office lives
// (its bims move) on its own loop, which already rests while the window is away (bims/room.ts);
// SideHost unmounts the world while the right side is hidden. As a 5th bubble (no room, d3) the
// same office draws at the bubble's 68 object. The office's labels and pills are the office tab's.
// Its camera (designer m_44998, E.4): folded, always the whole map, never a follow or a zoom,
// whoever he talks to; a beat (camera.ts: an agent just asks or changes state) shows only as its
// glass label over that bim's head. Grown, it opens at fit, plays no beats, and takes his input
// as the open map does (r2-side, Gabriel m_44190): a pinch zooms, two fingers or a drag while
// zoomed move it, through the office's own handlers (Office cameraFrom: the hit span over the
// island); folding back eases it to fit again (Office refit). While his pinch cuts the map the box
// becomes a small window onto the world (the frame: r 12, --c-sunken under the map, 1 --c-line,
// a soft shadow), faded in and out in 150 ms; the whole map has none. Owner: nm-side; the fit and
// the frame: m-bubbles.
import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { AppIcon, Press, tipAttrs } from '@/ds';
import { Office } from '@/bims/Office';
import { parts as bimsParts } from '@/bims/feed';
import { mapOf, mapPrefKey, type MapId } from '@/bims/maps/ids';
import { useOfficeObjects } from '@/bims/useOfficeObjects';
import { beatDelay, EASE_MS, happenedOf, HOLD_MS, pickBeat, seenOf, stillRecent, tickSoon, type Happened, type LastBeat, type Seen } from './camera';
import { speechOf, speechWord } from './model';
import type { Agent, Card } from '@/proto';
import { getPref, type Prefs } from '@/settings/prefs';
import { useGrowth, usePlaces, usePrefs } from '@/store';
import { usePlugins as usePluginRows } from '@/store';
import { side } from '@/proto/words_side.gen';

/** the hover that grows the world, ms (native-mac §7) */
export const HOVER_MS = 300;
/** the pointer gone, the big world waits this long before it folds (designer m_42536) */
export const GRACE_MS = 150;
/** the office's drawn width: the big world's; the small one is it scaled */
export const BIG = 384;
export const SMALL = 148;

/** No growth on hover: macOS's reduce motion, by its media query or main's flag on <html>. */
export function noGrow(): boolean {
  if (typeof document !== 'undefined' && document.documentElement.hasAttribute('data-reduce-motion')) return true;
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/** the bubbles' column, from the world's right edge (44 + its objects' 12 each side): the big world never takes the pointer there */
export const BUBBLE_COL = 68;
/** where the bubbles' objects start, from the world's top (the first bubble at 152 - 12, its object 28 above it) */
export const BUBBLE_FROM = 112;

/**
 * The world's pointer box, px in the world's own corner: the island as drawn (its 384 office
 * `isleH` tall, scaled to 148 at rest); grown, it leaves out the bubbles' column under BUBBLE_FROM,
 * so a bubble under the big world still takes his click. Pure.
 */
export function hitBox(isleH: number, big: boolean): CSSProperties {
  const k = big ? 1 : SMALL / BIG;
  const w = BIG * k;
  const h = Math.round(isleH * k);
  if (!big || h <= BUBBLE_FROM) return { width: w, height: h };
  const cut = `polygon(0 0, 100% 0, 100% ${String(BUBBLE_FROM)}px, calc(100% - ${String(BUBBLE_COL)}px) ${String(BUBBLE_FROM)}px, calc(100% - ${String(BUBBLE_COL)}px) 100%, 0 100%)`;
  return { width: w, height: h, clipPath: cut };
}

interface OfficeOf {
  readonly project: string;
  /** the project's live agents */
  readonly agents: readonly Agent[];
  readonly cards: readonly Card[];
}

/** A beat's label place, px in the folded world's box. */
type BeatAt = { readonly x: number; readonly y: number };
/** A beat: its agent and where its label goes (over its bim's head as the beat starts). */
interface Beat {
  readonly agent: string;
  readonly at: BeatAt;
}

/** When the beats play: not while grown (hovered) or while a float is open. */
interface BeatState {
  readonly hover: boolean;
  readonly frozen: boolean;
}

/**
 * The world's beats (camera.ts, designer m_42974; as labels only, designer m_44998): now and then
 * the agent that just did something, for EASE_MS + HOLD_MS; null the rest of the time. None while
 * grown, while a float is open, while the window is away or hidden, nor under reduced motion. The
 * clock restarts when the hover or the float ends. The camera never moves for one.
 */
function useWorldBeat(agents: readonly Agent[], { hover, frozen }: BeatState, placeOf: (agent: string) => BeatAt): Beat | null {
  const reduced = noGrow();
  const seen = useRef<ReadonlyMap<string, Seen> | null>(null);
  const happened = useRef<Happened[]>([]);
  // the clock's 'something just happened': its next tick pulled in (camera.ts tickSoon)
  const soon = useRef<() => void>(() => undefined);
  useEffect(() => {
    const next = seenOf(agents);
    const now = Date.now();
    const fresh = seen.current === null ? [] : happenedOf(seen.current, next, now);
    happened.current = [...stillRecent(happened.current, now), ...fresh];
    seen.current = next;
    if (fresh.length > 0) soon.current();
  }, [agents]);
  const [beat, setBeat] = useState<Beat | null>(null);
  const last = useRef<LastBeat | null>(null);
  const place = useRef(placeOf);
  useEffect(() => {
    place.current = placeOf;
  });
  const stopped = hover || frozen || reduced;
  useEffect(() => {
    if (stopped) return;
    let t: ReturnType<typeof setTimeout> | undefined;
    // when the next tick comes; null while a beat plays (its end arms the clock again)
    let tickAt: number | null = null;
    const armAt = (at: number) => {
      if (t !== undefined) clearTimeout(t);
      tickAt = at;
      t = setTimeout(fire, Math.max(0, at - Date.now()));
    };
    const arm = () => {
      armAt(Date.now() + beatDelay(Math.random()));
    };
    const fire = () => {
      const now = Date.now();
      const b = document.hidden ? null : pickBeat(happened.current, now, last.current);
      if (b === null) {
        arm();
        return;
      }
      last.current = { agent: b.agent, at: now };
      tickAt = null;
      setBeat({ agent: b.agent, at: place.current(b.agent) });
      // its label shows as long as a beat lasted when the camera went there, then the clock again
      t = setTimeout(() => {
        setBeat(null);
        arm();
      }, EASE_MS + HOLD_MS);
    };
    soon.current = () => {
      if (tickAt === null) return;
      const at = tickSoon(Date.now(), tickAt, last.current);
      if (at < tickAt) armAt(at);
    };
    // back from a hover or a float: a beat cut short goes, the clock starts again
    t = setTimeout(() => {
      setBeat(null);
      arm();
    }, 0);
    return () => {
      if (t !== undefined) clearTimeout(t);
    };
  }, [stopped]);
  return stopped ? null : beat;
}

/** The project's map, his /bims pick. */
function useMap(project: string): MapId {
  const prefs = usePrefs() as Prefs | null;
  return mapOf(getPref(prefs, mapPrefKey(project)));
}

/** What the world's office takes besides its rows: his camera's input while grown, the head finder, the zoom it reports, the refit. */
interface OfficeCam {
  readonly cameraFrom?: HTMLElement | null;
  readonly headRef?: { current: ((name: string) => { readonly x: number; readonly y: number } | null) | null };
  readonly onZoomed?: (zoomed: boolean) => void;
  readonly refit?: number;
  /** a click on a bim (the outer tile: its thread); none: nothing */
  readonly onOpen?: (agent: string) => void;
  /** his pick in the map chip (the outer tile); none: no chip */
  readonly onPickMap?: (map: MapId) => void;
  /** the office fills its holder's box, whole and centred (the outer tile) */
  readonly fill?: boolean;
  /** the bim the camera follows (the outer tile: the open agent's, bims/focus followOf); none: the whole map (the coins' world) */
  readonly follow?: string | null;
}

/** The project's office as the world draws it: its map, its plugins' objects, its growth; a follow camera only when given one. */
export function WorldOffice({ project, agents, cards, cameraFrom = null, headRef, onZoomed, refit, onOpen, onPickMap, fill = false, follow = null }: OfficeOf & OfficeCam) {
  const places = usePlaces(project);
  const growth = useGrowth(project);
  const map = useMap(project);
  // his plugins' objects, the plugins asked here too: the tile drew none until the Office tab asked (his m_49375)
  const objects = useOfficeObjects(project);
  // and the plugins rows: a working bim walks to the object of the plugin its call uses (uses.ts)
  const pluginRows = usePluginRows(project);
  return (
    <Office project={project} agents={agents} cards={cards} places={places} objects={objects} plugins={pluginRows} growth={growth} map={map}
      parts={bimsParts} transparent fill={fill} onOpen={onOpen ?? (() => undefined)} {...(onPickMap === undefined ? {} : { onPickMap })} follow={follow} cameraFrom={cameraFrom} headRef={headRef} onZoomed={onZoomed} refit={refit} />
  );
}

/**
 * The beat's name label (designer m_43213, m_44998): the agent's name and its state word, its
 * bottom over that bim's head on the whole map (`at`, px in the world's box; none found: over the
 * island's middle); outside the scaled island, so it reads at 148. For the beat's length. Pure render.
 */
function BeatLabel({ agent, cards, at }: { readonly agent: Agent; readonly cards: readonly Card[]; readonly at: { readonly x: number; readonly y: number } }) {
  // it asks him when it has open cards (the beat's own signal, camera.ts seenOf), even before its card is in `cards`
  const asks = agent.waits > 0;
  return (
    <span className="s3-beat" style={{ left: at.x, top: at.y }} data-testid="app.world.beat">
      <b>{agent.name}</b>
      <span className="s3-beat-says" data-asks={asks || undefined}>{asks ? side.state_asks : speechWord(speechOf(agent, cards))}</span>
    </span>
  );
}

export interface WorldProps extends OfficeOf {
  /** the project's name, as the sidebar says it */
  readonly name: string;
  /** the legend's words after the name ('3 at their desks · 1 asks') */
  readonly says: string;
  /** a click: the office in a tab */
  readonly onOpen: () => void;
  /** a float is open: no beat plays (camera.ts) */
  readonly frozen?: boolean;
}

/**
 * Where a beat's label goes, px in the folded world's box: over its bim's head on the whole map
 * (`head`, the office's canvas px at BIG, scaled to SMALL), else over the island's middle. Pure.
 */
export function beatPlace(head: { readonly x: number; readonly y: number } | null, isleH: number): { readonly x: number; readonly y: number } {
  const k = SMALL / BIG;
  if (head === null) return { x: Math.round(SMALL / 2), y: Math.round((isleH * k) / 2) };
  return { x: Math.round(head.x * k), y: Math.round(head.y * k) };
}

/** The world, small at rest, big after a hover. */
export function World({ project, name, says, agents, cards, onOpen, frozen = false }: WorldProps) {
  const [big, setBig] = useState(false);
  // where a bim's head is on the whole map (Office headRef), read as its beat starts
  const heads = useRef<((n: string) => BeatAt | null) | null>(null);
  const isleHRef = useRef(0);
  // folded, the whole map; a beat now and then as a label over its bim (camera.ts, designer m_44998)
  const beatOf = useWorldBeat(agents, { hover: big, frozen }, (agent) => beatPlace(heads.current?.(agent) ?? null, isleHRef.current));
  const beat = beatOf === null ? undefined : agents.find((a) => a.name === beatOf.agent);
  // folding back: the office eases to fit again (his pinch while grown is forgotten)
  const [refit, setRefit] = useState(0);
  const [wasBig, setWasBig] = useState(big);
  if (wasBig !== big) {
    setWasBig(big);
    if (!big) setRefit((n) => n + 1);
  }
  // his pinch cuts the map: the box is framed (designer m_44998)
  const [zoomed, setZoomed] = useState(false);

  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const stop = () => {
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = null;
  };
  useEffect(() => stop, []);
  const after = (ms: number, next: boolean) => {
    stop();
    timer.current = setTimeout(() => {
      timer.current = null;
      setBig(next);
    }, ms);
  };
  const enter = () => {
    if (noGrow()) return;
    after(big ? 0 : HOVER_MS, true);
  };
  const leave = () => {
    after(GRACE_MS, false);
  };
  // the office's drawn height at 384 (its layout box: the scale leaves it as it is)
  const isle = useRef<HTMLSpanElement | null>(null);
  const [isleH, setIsleH] = useState(0);
  useEffect(() => {
    const el = isle.current;
    if (el === null || typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(() => {
      setIsleH(el.offsetHeight);
      isleHRef.current = el.offsetHeight;
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
    };
  }, []);
  // the hit span: grown, his pinch, two fingers and drag there move the office's camera
  const [hit, setHit] = useState<HTMLSpanElement | null>(null);
  const label = side.world_open({ project: name });
  return (
    <Press look="s3-world" data-big={big || undefined} data-framed={(big && zoomed) || undefined} aria-label={label} {...tipAttrs(big ? undefined : label)}
      onPointerEnter={enter} onPointerLeave={leave} onFocus={enter} onBlur={leave} onClick={onOpen} data-testid="app.world">
      {/* the one place that takes the pointer: the island's box as drawn, never the bubbles' column under it */}
      <span ref={setHit} className="s3-world-hit" style={hitBox(isleH, big)} data-testid="app.world.hit" />
      <span ref={isle} className="s3-world-isle" style={{ '--s3-scale': String(big ? 1 : SMALL / BIG) } as CSSProperties}>
        {/* the frame: shown only while his pinch cuts the map (side3d.css .s3-world[data-framed]) */}
        <span className="s3-world-frame" aria-hidden="true" data-testid="app.world.frame" />
        <span className="s3-world-office" aria-hidden="true">
          <WorldOffice project={project} agents={agents} cards={cards} cameraFrom={big ? hit : null} headRef={heads} onZoomed={setZoomed} refit={refit} />
        </span>
      </span>
      {beat !== undefined && beatOf !== null && !big && <BeatLabel agent={beat} cards={cards} at={beatOf.at} />}
      {big && (
        <span className="s3-legend" data-testid="app.world.legend">
          <b>{name}</b>
          {says !== '' && <span className="s3-legend-says">{says}</span>}
          <span className="s3-legend-open" {...tipAttrs(side.world_open_tip)}>
            <AppIcon name="open" size={14} />
          </span>
        </span>
      )}
    </Press>
  );
}

/** The world folded into a 5th bubble: the same office at the bubble's object size. */
export function WorldThumb(props: OfficeOf) {
  return (
    <span className="s3-thumb" aria-hidden="true">
      <WorldOffice {...props} />
    </span>
  );
}
