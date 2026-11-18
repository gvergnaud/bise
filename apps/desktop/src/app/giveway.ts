// The title bar's give-way order when room runs short (designer m_45616, m_45674): the coins'
// minis and their counts never shrink; first the crumb's fact ('· waits for you') goes, then the
// status pill takes its short form ('updating to v2026.10.3…'), never a word cut mid-way; last the
// crumb's name cuts with … (the bar's flex: the title keeps its width before the slot). Pure:
// widths in, what gives way out; Frame's StatusSlot measures and applies. Owner: amb-win (the
// slot), nm-side (the minis in it).

/** Under this much room for the pill, its short form (designer's desktop-notices v2). */
export const STATUS_NARROW_PX = 160;

/** What the bar holds, px. */
export interface BarRoom {
  /** the status slot's width with the crumb's fact shown */
  readonly slot: number;
  /** the room the crumb's fact takes (its width and its gap), 0 without one */
  readonly fact: number;
  /** the pill's width with its long words whole, 0 without a pill */
  readonly pill: number;
  /** the minis' width with their margins, 0 without them */
  readonly minis: number;
}

/** What gives way: the fact hidden, the pill in its short form. */
export interface GiveWay {
  readonly fact: boolean;
  readonly narrow: boolean;
}

/** The give-way for that room. Pure. */
export function giveWay(r: BarRoom): GiveWay {
  if (r.pill <= 0) return { fact: false, narrow: false };
  const need = r.pill + r.minis;
  if (need <= r.slot) return { fact: false, narrow: r.slot - r.minis < STATUS_NARROW_PX };
  if (r.fact > 0 && need <= r.slot + r.fact) return { fact: true, narrow: false };
  return { fact: r.fact > 0, narrow: true };
}

/** What the crumb and the pill keep beside the minis, px. */
export interface CrumbRoom {
  /** the room the crumb and the status slot share */
  readonly avail: number;
  /** the crumb's name with its lead padding */
  readonly name: number;
  /** its role ('profiling the hub · acme') and its gap, 0 without one */
  readonly role: number;
  readonly minis: number;
  /** the pill's short form, and its sign alone; 0 without a pill */
  readonly pillShort: number;
  readonly pillSign: number;
}

/**
 * Beside the minis, nothing is cut mid-word (designer m_45727): the crumb's role shows whole or
 * not at all, before the pill's short form gives way to its sign alone; the name always stays.
 * `role`: the role shows; `pill`: the room the pill keeps (its short form, else its sign). Pure.
 */
export function cutCrumb(r: CrumbRoom): { readonly role: boolean; readonly pill: number } {
  if (r.minis <= 0) return { role: true, pill: r.pillShort };
  const role = r.role > 0 && r.name + r.role + r.minis + r.pillShort <= r.avail;
  const pill = r.name + (role ? r.role : 0) + r.minis + r.pillShort <= r.avail ? r.pillShort : r.pillSign;
  return { role, pill };
}

/** What the bar holds when the minis sit after the crumb (his m_45864, designer m_45869), px. */
export interface BarFit {
  /** the room the crumb, the minis and the status slot share */
  readonly avail: number;
  /** the crumb's name with its lead padding */
  readonly name: number;
  /** its role ('profiling the hub · acme') and its gap, 0 without one */
  readonly role: number;
  /** its fact ('? waits for you') and its gap, 0 without one */
  readonly fact: number;
  /** the minis with their margin: they never shrink */
  readonly minis: number;
  /** the pill whole, its short form, its sign alone; 0 without a pill */
  readonly pillFull: number;
  readonly pillShort: number;
  readonly pillSign: number;
}

/** What shows: the fact, the role, the pill's room and its short form. */
export interface BarFits {
  readonly fact: boolean;
  readonly role: boolean;
  readonly narrow: boolean;
  readonly pill: number;
}

/**
 * The minis right after the crumb (designer m_45869): they never shrink; first the crumb's fact goes,
 * then the pill takes its short form, then the crumb's role drops whole (never cut mid-word), then the
 * pill shows its sign alone; the name stays. Pure.
 */
export function barFit(r: BarFit): BarFits {
  const fits = (role: boolean, fact: boolean, pill: number) => r.name + (role ? r.role : 0) + (fact ? r.fact : 0) + r.minis + pill <= r.avail;
  if (fits(true, true, r.pillFull)) return { fact: true, role: true, narrow: false, pill: r.pillFull };
  if (fits(true, false, r.pillFull)) return { fact: false, role: true, narrow: false, pill: r.pillFull };
  if (fits(true, false, r.pillShort)) return { fact: false, role: true, narrow: r.pillFull > 0, pill: r.pillShort };
  if (fits(false, false, r.pillShort)) return { fact: false, role: false, narrow: r.pillFull > 0, pill: r.pillShort };
  return { fact: false, role: false, narrow: r.pillFull > 0, pill: r.pillSign };
}

/** A cut text keeps at least this: its … and a few letters, never a letter left over (designer m_57304). */
export const CUT_MIN_PX = 48;

/** The parts of a thread's title row (designer m_67200), in the order they give way: the first goes first. */
export const ROW_ORDER = ['sub', 'page', 'branch', 'pr', 'chip', 'fact', 'name'] as const;
export type RowKey = (typeof ROW_ORDER)[number];

/**
 * A part of the title row, px, its gap before it included; all 0 when it isn't there.
 * `least`: its narrowest cut (`whole` when it never cuts); `last`: what it keeps once it gave way
 * (0: out; the task chip its mark alone; the name its least: never out).
 */
export interface RowPart {
  readonly whole: number;
  readonly least: number;
  readonly last: number;
}

/** What the title can take (its content box and the status slot's slack), and each part. */
export type RowRoom = { readonly room: number } & { readonly [K in RowKey]: RowPart };

/** How a part shows: whole, cut to `width`, or in its last form (out, the chip's mark alone, the name at its least). */
export interface RowShow {
  readonly form: 'whole' | 'cut' | 'last';
  readonly width: number;
}
export type RowFits = { readonly [K in RowKey]: RowShow };

/**
 * The title row's give-way when room runs short (designer m_67200): each part in ROW_ORDER, one
 * at a time, gives the least it can: cut down to its least, else its last form, before the next
 * part gives anything. So the subtitle goes, then the page link, the branch (each cut, then out),
 * the PR out, the task chip cut then its mark alone (never out), the state fact out, the name cut
 * last. Every part shows within its width: the widths shown never add up past the room unless the
 * name at its least and the chip's mark alone are already more. Pure.
 */
export function rowFit(r: RowRoom): RowFits {
  let over = ROW_ORDER.reduce((n, k) => n + r[k].whole, 0) - r.room;
  const out = {} as Record<RowKey, RowShow>;
  for (const k of ROW_ORDER) {
    const p = r[k];
    if (over <= 0 || p.whole <= 0) out[k] = { form: 'whole', width: p.whole };
    else if (p.least < p.whole && over <= p.whole - p.least) {
      out[k] = { form: 'cut', width: p.whole - over };
      over = 0;
    } else {
      out[k] = { form: 'last', width: p.last };
      over -= p.whole - p.last;
    }
  }
  return out;
}
