// src/app/side3d · the bims' world over the outer card (page outer-world v2, his pick #799): the
// tile, the card's width (layout outerW, 292 by default) and as high as he left it (140 by default), r 12, the card's ring and e-1, its top
// 12 under the bar (tilestate OUTER_TOP, inline; his m_46677); the project's office at its fit, live: a click on a bim opens its
// thread; an agent's thread open (from the sidebar, the card or the bim), main's too, the camera centres on its bim and follows it,
// 6x close (bims/focus followOf, FOLLOW_ZOOM, his m_47261, m_47407, m_48928; his pinch or drag stops it until he opens another); an ask opened in place
// in the card looks at its bim while it is open (designer m_47300); a pinch zooms inside, the office's own 'Loft ⌄' chip bottom left and '⤢ fit' bottom right
// while zoomed. On hover two 24 px buttons top right: '⤢' the world in the panel's Office tab, '×'
// hides the tile (tilestate.ts keeps it per window). Under it, the 8 px gap to the card is the
// handle: row-resize, a 2 px text-3 line after 120 ms of hover (the panel's signed handle), a drag
// from 96 to half the window's height, a double-click back to 140. On bise's own (every project in
// scope) the tile holds the town (bims/town/Town.tsx) whole in its height, in place of one office,
// with no '⤢' (no town tab); it was the RightBar's until 6d9d07180 (his m_47415). While a new
// project's welcome shows in main's thread (welcome/shown.ts) the tile is its empty frame, and the
// welcome's office docks into it at his first send (welcome/dock.ts; designer m_47929). Owner: nm-side.
import { useEffect, useRef, useState, type PointerEvent as RPointerEvent, type RefObject } from 'react';
import type { Agent, Card } from '@/proto';
import type { MapId } from '@/bims/maps/ids';
import { Press, tipAttrs } from '@/ds';
import { side } from '@/proto/words_side.gen';
import { bims } from '@/proto/words_bims.gen';
import { Town } from '@/bims/town/Town';
import { townParts } from '@/bims/town/feed';
import { useTownProjects } from '@/bims/town/useTown';
import type { TilePhase } from '@/welcome/shown';
import { WorldOffice } from './World';
import { clampTile, OUTER_TOP, TILE_GAP, TILE_H, TILE_MIN, tileMax } from './tilestate';
import { OUTER_W } from './layout';

export interface OuterTileProps {
  readonly project: string;
  /** the project's name, as the sidebar says it */
  readonly name: string;
  /** the project's live agents */
  readonly agents: readonly Agent[];
  readonly cards: readonly Card[];
  /** its height now, css px */
  readonly h: number;
  /** its width, the card's (layout outerW; 292 by default) */
  readonly w?: number;
  /** the open agent's bim, the camera centres on it and follows it (bims/focus followOf); null: the whole office */
  readonly follow: string | null;
  /** a click on a bim: its thread */
  readonly onOpen: (agent: string) => void;
  /** his pick in the map chip */
  readonly onPickMap: (map: MapId) => void;
  /** '⤢': the world in the panel */
  readonly onPanel: () => void;
  /** '×': the tile goes */
  readonly onHide: () => void;
}

/** On bise's own (every project): the town in the tile, a click on an office dives in, 'open' its main thread, a bim its agent's. */
export interface TownActs {
  readonly onOpenProject: (project: string) => void;
  readonly onOpenAgent: (project: string, agent: string) => void;
}

/**
 * The tile: the office (on bise's own the town), '⤢' (the office alone: no town tab) and '×' on hover.
 * `phase` (welcome/shown.ts): 'welcome', a new project's welcome holds the office in main's thread
 * (one office on screen, native-lead m_47921): the frame alone at its place, its ground and line, no
 * office and no buttons (designer m_47929). 'docking', its copy flies in (welcome/dock.ts): the
 * office mounts at its fit under it, no follow, its chrome held (data-landing). Landed (the copy
 * goes the frame after that office drew): the chrome fades in (data-landed; designer m_47950), and
 * FOLLOW_BEAT_MS later the camera eases to the followed bim (designer m_47972).
 */
export function OuterTile({ project, name, agents, cards, h, w = OUTER_W, follow, onOpen, onPickMap, onPanel, onHide, town = null, phase = null }: OuterTileProps & { readonly town?: TownActs | null; readonly phase?: TilePhase }) {
  const [docked, setDocked] = useState(phase === 'docking');
  if (phase === 'docking' && !docked) setDocked(true);
  // the corner buttons: the town keeps its signs clear of them
  const actsRef = useRef<HTMLSpanElement>(null);
  // landed (the copy went the frame after the tile's office drew): the camera waits one beat, then finds the followed bim
  const [was, setWas] = useState(phase);
  const [beat, setBeat] = useState(false);
  if (was !== phase) {
    setWas(phase);
    if (was === 'docking' && phase === null) setBeat(true);
  }
  useEffect(() => {
    if (!beat) return undefined;
    const t = setTimeout(() => {
      setBeat(false);
    }, FOLLOW_BEAT_MS);
    return () => {
      clearTimeout(t);
    };
  }, [beat]);
  const waiting = phase === 'welcome';
  return (
    <section className="s3-otile" style={{ top: OUTER_TOP, height: h, width: w }} aria-label={town !== null ? bims.town_label : side.tile_name({ project: name })} data-testid="app.outer.world" data-town={town !== null || undefined} data-waiting={waiting || undefined} data-landing={phase === 'docking' || undefined} data-landed={(docked && phase === null) || undefined}>
      <span className="s3-otile-office">
        {waiting ? null : town !== null ? (
          <OuterTown h={h} acts={town} buttons={actsRef} />
        ) : (
          <WorldOffice project={project} agents={agents} cards={cards} fill follow={phase === 'docking' || beat ? null : follow} onOpen={onOpen} onPickMap={onPickMap} />
        )}
      </span>
      {!waiting && <span ref={actsRef} className="s3-otile-acts">
        {/* data-bims-keep: a plugin object's tip keeps clear of them (bims/objpills.ts placeObjTip, designer m_52074) */}
        {town === null && <Press look="s3-otile-btn" aria-label={side.tile_panel} {...tipAttrs(side.tile_panel)} onClick={onPanel} data-bims-keep="" data-testid="app.outer.world.panel">⤢</Press>}
        <Press look="s3-otile-btn" aria-label={side.tile_hide} {...tipAttrs(side.tile_hide)} onClick={onHide} data-bims-keep="" data-testid="app.outer.world.hide">×</Press>
      </span>}
    </section>
  );
}

/** The town (bims/town/Town.tsx, its rows from every project), whole in the tile's height. */
function OuterTown({ h, acts, buttons }: { h: number; acts: TownActs; buttons: RefObject<HTMLSpanElement | null> }) {
  const projects = useTownProjects();
  // the tile's '×' over the town: no project sign or pill under it, 4 px clear (designer m_49471)
  return <Town projects={projects} parts={townParts} maxH={h} onOpenProject={acts.onOpenProject} onOpenAgent={acts.onOpenAgent} keepClear={buttons} />;
}

/** after the welcome's dock, the tile's office drawn: the camera eases to the followed bim this long after, ms (designer m_47972) */
export const FOLLOW_BEAT_MS = 200;

/** the hover before the handle's line shows, ms (the panel's signed handle) */
export const HANDLE_HOT_MS = 120;

/** The gap between the tile and the card: a drag sets the tile's height (96 .. half the window), a double-click 140; ↑ ↓ by 8. */
export function TileHandle({ h, w = OUTER_W, winH, onHeight }: { h: number; w?: number; winH: number; onHeight: (h: number) => void }) {
  const at = useRef<{ readonly y: number; readonly h: number } | null>(null);
  const [hot, setHot] = useState(false);
  const [drag, setDrag] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const cool = () => {
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = null;
  };
  const down = (e: RPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    at.current = { y: e.clientY, h };
    setDrag(true);
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      // a pointer the page no longer knows (a synthetic one): the drag still follows the moves over the handle
    }
    document.documentElement.classList.add('s3-resizing');
  };
  const move = (e: RPointerEvent<HTMLDivElement>) => {
    const a = at.current;
    if (a === null) return;
    onHeight(clampTile(a.h + e.clientY - a.y, winH));
  };
  const up = () => {
    at.current = null;
    setDrag(false);
    document.documentElement.classList.remove('s3-resizing');
  };
  return (
    <div
      className="s3-ohandle"
      // its place: the 8 under the tile, following the height
      style={{ top: OUTER_TOP + h, height: TILE_GAP, width: w }}
      role="separator"
      aria-orientation="horizontal"
      aria-label={side.tile_height}
      aria-valuemin={TILE_MIN}
      aria-valuemax={tileMax(winH)}
      aria-valuenow={h}
      tabIndex={0}
      data-hot={hot || undefined}
      data-drag={drag || undefined}
      {...tipAttrs(side.tile_height)}
      onPointerEnter={() => {
        cool();
        timer.current = setTimeout(() => {
          setHot(true);
        }, HANDLE_HOT_MS);
      }}
      onPointerLeave={() => {
        cool();
        setHot(false);
      }}
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={up}
      onDoubleClick={() => {
        onHeight(TILE_H);
      }}
      onKeyDown={(e) => {
        if (e.key === 'ArrowDown') onHeight(clampTile(h + 8, winH));
        else if (e.key === 'ArrowUp') onHeight(clampTile(h - 8, winH));
      }}
      data-testid="app.outer.world.handle"
    />
  );
}
