// macOS notifications for the questions bise asks (the user's decision 2026-10-09, card #476: « b +
// la notification native ... Pas de notification nécessaire si j'étais focused sur l'app au moment de
// la réception »; page inbox-native v2; designer m_14211's words). Pure: which cards are new, whether
// one knocks, what its notification says and offers, what a button or a reply answers. The shell is
// run.ts. Owner: amb-mac.
import type { Card } from '../../proto';
import { cardRoute } from '../../shared/asks';

/** The words, en and fr (designer m_14211). */
export const WORDS = {
  en: {
    asks: 'asks',
    proposes: 'proposes a plan',
    archive: (who: string) => `archive ${who}?`,
    open: 'open in bise',
    reply: 'Reply',
    placeholder: 'your answer',
    send: 'Send',
    later: 'later',
    failed: (who: string) => `couldn't send your answer to ${who}. it's still in bise's inbox.`,
  },
  fr: {
    asks: 'demande',
    proposes: 'propose un plan',
    archive: (who: string) => `archiver ${who} ?`,
    open: 'ouvrir dans bise',
    reply: 'Répondre',
    placeholder: 'ta réponse',
    send: 'Envoyer',
    later: 'plus tard',
    failed: (who: string) => `ta réponse à ${who} n'est pas partie. elle t'attend dans l'inbox de bise.`,
  },
} as const;
export type Lang = keyof typeof WORDS;

/** His language from the system's preferred languages: fr when the first is French, else en. */
export function langOf(preferred: readonly string[]): Lang {
  return (preferred[0] ?? '').toLowerCase().startsWith('fr') ? 'fr' : 'en';
}

/** A choice longer than this, or more than MAX_CHOICES: the menu only opens it in bise. */
export const MAX_CHOICE = 40;
export const MAX_CHOICES = 4;

/** What one notification shows and offers. */
export type Note = {
  /** project#id: the card it stands for */
  readonly key: string;
  readonly project: string;
  readonly card: number;
  readonly title: string;
  readonly body: string;
  /** the buttons, in order: the card's choices, then 'open in bise' for a question */
  readonly actions: readonly string[];
  /** the card option each button answers, null for 'open in bise' */
  readonly picks: readonly (number | null)[];
  /** a question takes typed words; an approval never does */
  readonly reply: boolean;
  readonly placeholder: string;
  /** a quiet moment: no sound, straight to Notification Center */
  readonly silent: boolean;
  /** group: the project */
  readonly group: string;
};

/** A project's facts the title needs (from the core's projects rows). */
export type Place = { readonly name: string; readonly home: boolean };

export const cardKey = (project: string, id: number): string => `${project}#${id}`;

/** '<agent> asks · <project>': main speaks as bise; an agent of bise's home names no project; a drop
 * card names what it archives. */
export function titleOf(c: Card, place: Place | undefined, lang: Lang): string {
  const who = c.agent === 'main' || c.agent === '' ? 'bise' : c.agent;
  const where = place === undefined || place.home ? '' : ` · ${place.name}`;
  // main's drop card is about its agent (drop-card, designer m_35655): 'archive sculpt-engine?'
  if (c.kind === 'drop') return `${WORDS[lang].archive(c.agent)}${where}`;
  // a plan an agent proposes (plan-ok): 'lean-install proposes a plan'
  return `${who} ${c.kind === 'plan' ? WORDS[lang].proposes : WORDS[lang].asks}${where}`;
}

/** The notification for a card (designer m_14211). */
export function noteOf(c: Card, place: Place | undefined, lang: Lang, quiet: boolean): Note {
  const w = WORDS[lang];
  const approval = c.approval === true;
  const fits = c.options.length >= 1 && c.options.length <= MAX_CHOICES && c.options.every((o) => o.label.length <= MAX_CHOICE);
  let actions: string[];
  let picks: (number | null)[];
  if (approval) {
    // its option words as the buttons; the click on the banner opens it, no 'open in bise'
    actions = c.options.map((o) => o.label);
    picks = c.options.map((o) => o.n);
  } else if (fits) {
    actions = [...c.options.map((o) => o.label), w.open];
    picks = [...c.options.map((o) => o.n), null];
  } else {
    actions = [w.open];
    picks = [null];
  }
  return {
    key: cardKey(c.project, c.id),
    project: c.project,
    card: c.id,
    title: titleOf(c, place, lang),
    body: c.question,
    actions,
    picks,
    reply: !approval,
    placeholder: w.placeholder,
    silent: quiet,
    group: c.project,
  };
}

/** What his gesture on a notification does. */
export type Gesture = { readonly action: number } | { readonly reply: string } | { readonly click: true };
export type Effect =
  | { readonly t: 'answer'; readonly cmd: { cmd: 'answer'; project: string; card: number; reply: string } }
  | { readonly t: 'open'; readonly route: string }
  | { readonly t: 'none' };

/** The window's route that opens this card in place in the inbox (amb-home's InboxBlock, m_14345): shared/asks.ts's. */
export { cardRoute };

/** A button answers with its option (as 1-4 in the window), a reply with his words, the rest opens that card in the inbox. */
export function effectOf(n: Note, g: Gesture): Effect {
  const open: Effect = { t: 'open', route: cardRoute(n.project, n.card) };
  if ('click' in g) return open;
  if ('reply' in g) {
    const words = g.reply.trim();
    if (!n.reply || words === '') return { t: 'none' };
    return { t: 'answer', cmd: { cmd: 'answer', project: n.project, card: n.card, reply: words } };
  }
  const pick = n.picks[g.action];
  if (pick === undefined) return { t: 'none' };
  if (pick === null) return open;
  return { t: 'answer', cmd: { cmd: 'answer', project: n.project, card: n.card, reply: String(pick) } };
}

/**
 * The cards of every project, as the core says them: a project's first list is what already
 * waited (no knock), every later card is new; a card gone from its list was answered (anywhere:
 * the window, the terminal, a notification) and its notification goes.
 */
export class Arrivals {
  private readonly open = new Map<string, Set<string>>();

  /** One project's open cards now: the new ones and the keys that left. */
  apply(project: string, cards: readonly Card[]): { readonly fresh: Card[]; readonly gone: string[] } {
    const now = new Set(cards.map((c) => cardKey(project, c.id)));
    const before = this.open.get(project);
    this.open.set(project, now);
    if (before === undefined) return { fresh: [], gone: [] };
    return {
      fresh: cards.filter((c) => !before.has(cardKey(project, c.id))),
      gone: [...before].filter((k) => !now.has(k)),
    };
  }
}

/** a0's test notification (Help › 'send a test notification'): like a real card, two choices and a reply. */
export function testNote(lang: Lang): Note {
  const n = noteOf(
    {
      id: 0,
      project: 'acme',
      kind: 'question',
      agent: 'perf',
      question: 'which bench? (a test notification: pick one, or Reply)',
      options: [
        { n: 1, label: 'cold' },
        { n: 2, label: 'warm' },
      ],
      urgent: false,
      since_ms: 0,
    },
    { name: 'acme', home: false },
    lang,
    false,
  );
  // two choices and Reply, no 'open in bise' (the test answers nobody)
  return { ...n, key: 'test', actions: n.actions.slice(0, 2), picks: n.picks.slice(0, 2) };
}

/** What macOS keeps for one app in com.apple.ncprefs (`defaults export com.apple.ncprefs -`, XML). */
export type NcPrefs = { readonly flags: number; readonly auth: number | null; readonly style: 'none' | 'banners' | 'alerts' | 'unknown' };

/**
 * His notification settings for `bundleId`, read from ncprefs' XML: null when macOS has no entry
 * (it never registered the app). The style is flags bits 3-5 (1 banners, 2 alerts, 0 none); the
 * raw flags are kept, the bits are macOS's private layout.
 */
export function ncPrefsOf(xml: string, bundleId: string): NcPrefs | null {
  const at = xml.indexOf(`<string>${bundleId}</string>`);
  if (at < 0) return null;
  const start = xml.lastIndexOf('<dict>', at);
  const end = xml.indexOf('</dict>', at);
  if (start < 0 || end < 0) return null;
  const d = xml.slice(start, end);
  const num = (key: string): number | null => {
    const m = new RegExp(`<key>${key}</key>\\s*<integer>(-?\\d+)</integer>`).exec(d);
    return m?.[1] === undefined ? null : Number(m[1]);
  };
  const flags = num('flags');
  if (flags === null) return null;
  const bits = (flags >> 3) & 7;
  const style = bits === 0 ? 'none' : bits === 1 ? 'banners' : bits === 2 ? 'alerts' : 'unknown';
  return { flags, auth: num('auth'), style };
}

/** The bundle id in an app's Info.plist (XML), or null. */
export function bundleIdOf(plist: string): string | null {
  return /<key>CFBundleIdentifier<\/key>\s*<string>([^<]+)<\/string>/.exec(plist)?.[1] ?? null;
}

/** Whether a new card knocks: only when no bise window had the keys at its arrival. */
export const knocks = (focused: boolean): boolean => !focused;
