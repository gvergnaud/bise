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

/** The subtitle cut no shorter than this: its … and a few letters, never a letter left over (designer m_57304). */
export const SUB_MIN_PX = 48;

/** A thread's title row in the bar (designer m_57254, m_57304), px, each with its gap before it. */
export interface TitleRoom {
  /** what the title can take: its own content box and the status slot's slack */
  readonly room: number;
  /** the name, and the state fact (0 when hidden): they stay */
  readonly fixed: number;
  /** the agent's links in order (PR, branch, page), each whole */
  readonly links: readonly number[];
  /** the subtitle whole, 0 without one */
  readonly sub: number;
}

/** What shows: the subtitle whole, cut with … or out; the first `links` links. */
export interface TitleFits {
  readonly sub: 'whole' | 'cut' | 'out';
  readonly links: number;
}

/**
 * The title's give-way when room runs short (designer m_57304): the subtitle first, cut with …
 * down to SUB_MIN_PX, then dropped whole; then the links whole, last first (page, branch; the PR
 * last); the name and the fact stay. Pure.
 */
export function titleFit(r: TitleRoom): TitleFits {
  const sum = (n: number) => r.links.slice(0, n).reduce((a, b) => a + b, 0);
  const all = r.links.length;
  if (r.fixed + sum(all) + r.sub <= r.room) return { sub: r.sub > 0 ? 'whole' : 'out', links: all };
  if (r.sub > SUB_MIN_PX && r.fixed + sum(all) + SUB_MIN_PX <= r.room) return { sub: 'cut', links: all };
  let n = all;
  while (n > 0 && r.fixed + sum(n) > r.room) n -= 1;
  return { sub: 'out', links: n };
}
