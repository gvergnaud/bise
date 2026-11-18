// The corners' test fixtures (tests only, never the app's runtime): a set of plugin objects, a
// crew, and every break of the corners' laws in one office. They lived in corners.test.ts, and
// six test files imported them from there, so each one ran the whole corners suite again (60 s
// at load 75: bims/objpills.test.ts's own 5 s limit went red on a corners test).
import { agent, mainAgent } from './fixtures';
import type { Box, Office } from './model';
import { boxLegGap } from './inbox';
import { network } from './paths';
import { ALSO, CORNER, WALK_GAP, type Corners } from './corners';
import type { PluginObject } from './plugin-objects';
import type { ObjId } from './render/objects/ids';
import { terraceSlab } from './render/decor';

/** a set of plugin objects: Gabriel's own first (page bims-plugins v15), then the collection's */
const MINE: readonly ObjId[] = [
  'library', 'files', 'visio', 'booth', 'arcade', 'pingpong', 'scooter', 'mailbox', 'easel', 'macdesk', 'printer', 'calendar', 'telescope', 'trophies', 'packing',
  'extinguisher', 'charts', 'cat', 'cushion', 'rack', 'shelf', 'plant', 'chess', 'piano', 'jukebox', 'aquarium', 'babyfoot', 'dog', 'bike', 'campfire', 'pinball', 'catwalk', 'globe', 'espresso',
];
export const objsOf = (n: number, from: readonly ObjId[] = MINE): PluginObject[] =>
  Array.from({ length: n }, (_, i) => ({ key: `p${String(i).padStart(2, '0')}-${from[i % from.length] as string}`, names: [`p${String(i)}`], obj: from[i % from.length] as ObjId, glass: false, pill: null, picked: false }));

export const crewOf = (n: number) => [mainAgent(), ...Array.from({ length: Math.max(0, n - 1) }, (_, i) => agent(`a${String(i + 1)}`))];

const overlaps = (a: Box, b: Box): boolean => a.x0 < b.x1 - 1e-9 && a.x1 > b.x0 + 1e-9 && a.z0 < b.z1 - 1e-9 && a.z1 > b.z0 + 1e-9;
const inside = (a: Box, b: Box): boolean => a.x0 >= b.x0 - 1e-9 && a.x1 <= b.x1 + 1e-9 && a.z0 >= b.z0 - 1e-9 && a.z1 <= b.z1 + 1e-9;

/** every break of the laws in one office's corners, a line each */
export function breaksOf(o: Office, c: Corners): string[] {
  const out: string[] = [];
  const legs = network(o);
  const cz = o.cozy;
  const kept: Box[] = [...o.furniture, ...(cz ? [cz.coffee, cz.shelf, ...cz.lamps, ...cz.plants].map((it) => it.box) : [])];
  const room = { x0: o.x0, x1: o.x0 + o.W, z0: 0, z1: o.D };
  c.placed.forEach((p, i) => {
    const at = `${o.map} ${p.obj}@${p.x.toFixed(2)},${p.z.toFixed(2)}`;
    const gap = Math.min(...legs.map((l) => boxLegGap(p.box, l)));
    if (gap < WALK_GAP - 1e-6) out.push(`${at}: a walk ${gap.toFixed(2)} from it`);
    if (kept.some((f) => overlaps(p.box, f))) out.push(`${at}: on furniture`);
    if (o.inbox && overlaps(p.box, { x0: o.inbox.x - 0.23, x1: o.inbox.x + 0.23, z0: o.inbox.z - 0.18, z1: o.inbox.z + 0.18 })) out.push(`${at}: on the inbox`);
    c.placed.slice(i + 1).forEach((q) => {
      if (overlaps(p.box, q.box)) out.push(`${at}: through ${q.obj}`);
    });
    // a grown office (extensions.ts): the wing is room, the garden terrace
    const floors = p.corner === 'terrace' ? [terraceSlab(o)] : p.corner === 'garden' ? (o.garden ? [o.garden] : []) : [room, ...(o.wing ? [o.wing] : [])];
    // the room and its wing are one floor (an L): a thing may stand across where they meet, never past the wing's front
    const w = o.wing;
    const acrossWing = w !== undefined && p.corner !== 'terrace' && p.corner !== 'garden' && inside(p.box, { x0: Math.min(room.x0, w.x0), x1: Math.max(room.x1, w.x1), z0: 0, z1: w.z1 });
    if (!acrossWing && !floors.some((f) => inside(p.box, f))) out.push(`${at}: off its ${p.corner === 'terrace' ? 'terrace' : 'room'}`);
    // only its own corner, or the other one a travel thing may take (the door or the terrace), or its swap's own (the ping-pong's babyfoot by the nap place)
    // a grown office's garden takes the terrace's things and the travel ones (extensions.ts, designer m_41450)
    const out_ = p.corner === 'garden' && (CORNER[p.of] === 'terrace' || ['mailbox', 'scooter', 'bike'].includes(p.of));
    if (!out_ && p.corner !== CORNER[p.of] && p.corner !== ALSO[p.of] && !(p.obj !== p.of && p.corner === CORNER[p.obj])) out.push(`${at}: ${CORNER[p.of]} thing in ${p.corner}`);
    if (p.corner === 'hang' && p.box.z0 > 0.3 && p.box.x0 > o.x0 + 0.3) out.push(`${at}: hangs off the back and left walls`);
  });
  return out;
}
