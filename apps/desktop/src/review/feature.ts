// A project's features (bar A.6, amb-feed 5f436473): how the sidebar groups
// their agents (the TUI's 'ψ gift-cards' group), the words of a feature's
// state, and its card's answers (try / merge / drop: no typed command, the
// feature card answers through store.answer, architect m_10203). Pure.
// Owner: amb-win.
import type { Agent, Card, Feature, OptAct } from '@/proto';

export interface Group {
  readonly feature: Feature;
  /** its live agents, in the hub's order */
  readonly agents: readonly Agent[];
}

/**
 * The panel's agents as the TUI draws them: the ones in no feature first,
 * then one group per feature (in the hub's order) with its agents. `live`
 * is the panel's list, main out.
 */
export function groupsOf(live: readonly Agent[], features: readonly Feature[]): { solo: Agent[]; groups: Group[] } {
  const inFeature = new Set(features.flatMap((f) => f.agents));
  const solo = live.filter((a) => !inFeature.has(a.name));
  const groups = features.map((f) => ({ feature: f, agents: live.filter((a) => f.agents.includes(a.name)) }));
  return { solo, groups };
}

/** Its state in a few words, the most current first. */
export function stateWords(f: Feature): string {
  if (f.building) return 'building a try';
  if (f.trial) return f.tried ? `on trial · ${f.tried.sha}` : 'on trial';
  if (f.checked) return 'checked · ready to try';
  if (f.ahead === 0) return 'nothing yet';
  return 'at work';
}

/** How far it is from its base, as the changes rows say it: '2 ahead · 1 behind · +40 −3'. */
export function distance(f: Feature): string {
  const parts = [`${String(f.ahead)} ahead`];
  if (f.behind > 0) parts.push(`${String(f.behind)} behind`);
  if (f.adds > 0 || f.dels > 0) parts.push(`+${String(f.adds)} −${String(f.dels)}`);
  return parts.join(' · ');
}

/** The feature's open card, if the store has it. */
export function cardOf(f: Feature, cards: readonly Card[]): Card | undefined {
  return f.card == null ? undefined : cards.find((c) => c.id === f.card);
}

export interface Choice {
  /** the digit sent as the answer; none: a local step (the drop's second ask) */
  readonly reply?: string;
  readonly label: string;
  /** asks 'drop it?' once more first */
  readonly confirm?: true;
  /** goes back from the drop's ask */
  readonly back?: true;
  /** done here, nothing sent, the card stays (the hub's row: the try card's '2 show the diff' opens its diff) */
  readonly act?: OptAct;
  /** its option's number (an act's: its test id) */
  readonly n?: number;
}

/** The merge card's option that drops the branch (the TUI's '3 drop the branch'). */
function isDrop(card: Card, n: number, label: string): boolean {
  return card.kind === 'feature_merge' && (n === 3 || /\bdrop\b/i.test(label));
}

/**
 * What its card offers: each option answers with its digit, except the
 * merge card's drop, which asks once more (dev-flow §5.1, the TUI's
 * drop_ask_shape): 'drop <name>? <n> commits go.' with 'drop it' (sends the
 * drop's digit) and 'keep it' (back).
 */
export function choicesOf(card: Card, asking: boolean): Choice[] {
  if (asking) {
    const drop = card.options.find((o) => isDrop(card, o.n, o.label));
    return [{ reply: String(drop?.n ?? 3), label: 'drop it' }, { label: 'keep it', back: true }];
  }
  return card.options.map((o) => {
    if (o.act != null) return { act: o.act, label: o.label, n: o.n };
    return isDrop(card, o.n, o.label) ? { label: o.label, confirm: true } : { reply: String(o.n), label: o.label };
  });
}

/** The drop's second ask. */
export function dropAsk(f: Feature): string {
  return `drop ${f.name}? ${String(f.ahead)} commit${f.ahead === 1 ? '' : 's'} go.`;
}
