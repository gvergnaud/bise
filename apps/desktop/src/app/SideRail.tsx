// The rail (bise fait mac, page native-mac §6, mockup 1): always there, 56 wide, transparent on the
// window's vibrancy, under the bar. bise's ':*' (mono 15/600: its thread), inbox, artifacts,
// scheduled, processes (every running route; no dot: nothing there waits on him, designer
// m_55616), plugins; settings at the bottom. Buttons 36 × 36 r 8, 4 apart, 8 from the top and 16
// from the bottom; the line icons 18 (ds AppIcon); the current place --c-active and --c-text, the
// others text-2; inbox's accent dot (8, at 6/6) only while something waits on him; each button a
// tip with its key, and ⌘ held (held.ts, its one delay) the key's cap in place of its icon, as the
// bar's sidebar button (rail-keys, designer m_45492). It replaces the old 'rail under 1200 px with an artifact open'.
// bise's button from a page goes back to his last project or chat, from bise's tab to its own
// thread (railback.ts, his m_49841). Owner: nm-frame.
import type { ReactNode } from 'react';
import { AppIcon, type AppIconName, Kbd, Press } from '@/ds';
import { PLACE_KEYS } from '@/settings/keys';
import { frame } from '@/proto/words_frame.gen';
import { panel } from '@/proto/words_panel.gen';
import { useStore } from '@/store';
import { biseBack, RAIL_GO, railPlaceOf, type RailPlace } from './railback';
import { useRoute, useRouter } from './router';
import { useModHeld } from './held';
import { useWaitingTotal } from './Sidebar';
import { waitsTip } from './tipwords';

/** Each place's key (app/keys.ts opens it): its tip, its held cap, the Go menu. */
export const RAIL_KEYS: Record<RailPlace, string> = {
  bise: PLACE_KEYS.bise,
  inbox: PLACE_KEYS.inbox,
  scheduled: PLACE_KEYS.scheduled,
  artifacts: PLACE_KEYS.artifacts,
  processes: PLACE_KEYS.processes,
  plugins: PLACE_KEYS.plugins,
  settings: PLACE_KEYS.settings,
};

const NAMES: Record<RailPlace, string> = {
  bise: panel.bise,
  inbox: frame.inbox,
  scheduled: frame.scheduled,
  artifacts: frame.artifacts,
  processes: frame.processes,
  plugins: frame.plugins,
  settings: frame.settings,
};

/** A place's tip: its name and its key ('inbox · ⌘I'). Pure. */
export const railTip = (p: RailPlace): string => frame.tip_key({ what: NAMES[p], key: RAIL_KEYS[p] });

function RailButton({ place, on, children, dot, tip }: { place: RailPlace; on: boolean; children: ReactNode; dot?: boolean; tip?: string }) {
  const router = useRouter();
  const store = useStore();
  // ⌘ held: the key's cap in place of the icon (one cap, never stacked over it; designer m_45492)
  const held = useModHeld('Meta');
  return (
    <Press
      look={on ? 'app-rail-b on' : 'app-rail-b'}
      aria-label={NAMES[place]}
      aria-current={on ? 'page' : undefined}
      data-tip={tip ?? railTip(place)}
      onClick={() => {
        // bise's: back to his last project or chat (railback.ts, his m_49841)
        router.go(place === 'bise' ? biseBack.go(router.get(), { projects: store.projects(), agents: (p) => store.agents(p) }) : RAIL_GO[place]);
      }}
      data-testid={`app.rail.${place}`}
      data-held={held || undefined}
    >
      {held ? (
        <span className="app-keyhint app-rail-key" data-testid={`app.rail.${place}.key`}>
          <Kbd>{RAIL_KEYS[place]}</Kbd>
        </span>
      ) : (
        children
      )}
      {/* ⌘ held, the dot stays on the button's top-right corner, 2 clear of the cap (app.css; designer m_45574) */}
      {dot === true && <span className="app-rail-dot" data-testid={`app.rail.${place}.dot`} />}
    </Press>
  );
}

const ICON: Record<Exclude<RailPlace, 'bise'>, AppIconName> = {
  inbox: 'inbox',
  scheduled: 'scheduled',
  artifacts: 'artifacts',
  // the right side's processes tile's, so one icon means processes everywhere (designer m_55616)
  processes: 'terminal',
  plugins: 'plugins',
  settings: 'settings',
};

/** The rail's places between bise's and settings, in order: the work places together (designer m_55616). */
export const RAIL_MIDDLE = ['inbox', 'artifacts', 'scheduled', 'processes', 'plugins'] as const satisfies readonly RailPlace[];

export function SideRail() {
  const at = railPlaceOf(useRoute());
  const waits = useWaitingTotal();
  return (
    <nav className="app-rail" aria-label={frame.places} data-testid="app.rail">
      <RailButton place="bise" on={at === 'bise'}>
        <span className="app-rail-mark" aria-hidden="true">
          :*
        </span>
      </RailButton>
      {RAIL_MIDDLE.map((p) => (
        <RailButton key={p} place={p} on={at === p} {...(p === 'inbox' ? { dot: waits > 0, tip: waits > 0 ? `${railTip(p)} · ${waitsTip(waits, true)}` : railTip(p) } : {})}>
          <AppIcon name={ICON[p]} size={18} />
        </RailButton>
      ))}
      <span className="app-rail-gap" />
      <RailButton place="settings" on={at === 'settings'}>
        <AppIcon name="settings" size={18} />
      </RailButton>
    </nav>
  );
}
