// A plugin's icon in the plugins place: one tile for every icon (ui-review #4: the same frame and
// mass), its office object inside (bims-link's objectOf(row, picks), drawn by render.ts through
// bims-objects' makeObject at its shown size x the screen's ratio, 2x at least), dimmed when off, with its '?' / '✗' badge, each explaining
// itself on hover. Until the object is drawn, and for good without WebGL or for a row that brings
// none, the same tile holds the title's capital initial (MonoTile, also the band's for a connector
// with no row). Takes the row as is (bims-link m_40142: its pick, its count and its badges come
// from the row's typed fields). Owner: plug-desktop.
import { tipAttrs } from '@/ds';
import type { Plugin } from '@/proto';
import { settings } from '@/proto/words.gen';
import { useIconUrl, usePicks } from './icons';
import { badgeOf, titleOf } from './model';
import { iconObject, iconScale } from './spots';

/** the object's css size: a row's (in its tile 40), the plugin page's head (shown in the banner's
 * visual box at its full height, tokens.css --h-banner-vis 176; drawn smaller it was upscaled and
 * blurred, Gabriel m_52286) */
const PX = { row: 32, head: 176 } as const;

/** device px per css px for this screen's icons (2x at least) */
const scaleNow = (): number => iconScale(typeof window === 'undefined' ? 1 : window.devicePixelRatio);

/** 'Linear' -> 'L', 'image generation' -> 'I', 'élan' -> 'É', '' -> '?'. Pure. */
export function initialOf(title: string): string {
  const c = /[\p{L}\p{N}]/u.exec(title)?.[0];
  return c === undefined ? '?' : c.toUpperCase();
}

/** The tile with a title's initial: a row with no object, a band card with no row. */
export function MonoTile({ title, size = 'row', testId }: { title: string; size?: 'row' | 'head'; testId?: string }) {
  return (
    <span className={`plg-icon ${size}`} aria-hidden="true" data-testid={testId}>
      <span className="plg-mono-letter">{initialOf(title)}</span>
    </span>
  );
}

/** `lit`: drawn as on though the row is off (computer, a built-in not set up yet: on, says Gabriel). */
export function PluginIcon({ row, project, size = 'row', lit = false, testId }: { row: Plugin; project?: string; size?: 'row' | 'head'; lit?: boolean; testId?: string }) {
  const badge = badgeOf(row);
  const picks = usePicks(project);
  const o = iconObject(row, picks, lit);
  // off: drawn solid and dimmed by the tile (.glass, half), never in glass, which all but vanished
  // in dark (ui-review #15); the room keeps its glass
  const url = useIconUrl(o === null ? null : { ...o, glass: false }, PX[size], scaleNow());
  return (
    <span className={`plg-icon ${size}${row.on || lit || row.state === 'available' ? '' : ' glass'}`} data-kind={row.kind} data-obj={url === null ? undefined : o?.obj}
      aria-hidden={badge === null || undefined} data-testid={testId}>
      {url !== null ? <img className="plg-obj" src={url} alt="" draggable={false} /> : <span className="plg-mono-letter">{initialOf(titleOf(row))}</span>}
      {badge !== null && (
        <span className={`plg-badge${badge === '✗' ? ' err' : ''}`} {...tipAttrs(badge === '✗' ? settings.plugins_badge_failed : settings.plugins_badge_waits)}>
          {badge}
        </span>
      )}
    </span>
  );
}
