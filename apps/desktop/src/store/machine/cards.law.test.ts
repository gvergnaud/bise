// The cards slice's law (fast-try p4 slice 1, architect m_29198/m_29689, review-refactor m_29693):
// seeded random runs of the hubs' rows, his answers, closes and retries, and main's answers to
// them, through stepCards; after each input the laws below hold. The runs reach the edges where the
// bugs were: a project not loaded yet (no row, or a row without its cards field) then rows that
// still list the card, the clock jumping across the 60 s stale mark (exactly on it too), more than
// 50 cards acted on, two projects with the same card ids, main's answer before and after the rows
// that drop the card. A red prints its seed and step: `SEED=<n> pnpm vitest run cards.law` replays it.
//
// The spec model below says the rules in their own words (independent of acted.ts / refused.ts):
// - a card he answered or closed is his act in flight until the hub settles it: its loaded row no
//   longer lists it, or still lists it 60 s on (the hub answers in ms), or main says the hub took
//   it while its loaded row still lists it (kept open), or main says it was refused; at most the
//   newest 50 are held;
// - a refusal (its words, what he sent) holds until he acts on that card again, the hub takes it,
//   or its loaded row no longer lists it; at most the newest 50;
// - where he made an answer is kept once the hub took it (an answer, not {}), from a thread or the
//   inbox screen; the newest 200, one per card;
// - the window goes to the asking project's main after an answer the hub took, made from another
//   project's thread, unless the card is bise's own (the home project's).
import { describe, expect, it } from "vitest";
import type { Artifact, Card, CardAnswer, Part, ProjectRow } from "@/proto";
import type { OptAct } from "@/proto/rows";
import { core_ev } from "@/proto/fixtures.gen";
import type { CoreCmd } from "@/shared/api";
import { cardsInit, inboxView, stepCards, type CardsState } from "./cards";
import { NO_FACTS, factsOf, type Effect, type HubFacts, type Input, type Output } from "./types";
import type { AnswerFrom } from "../answered";

// ---- a seeded generator (mulberry32): a red reproduces from its seed

function rng(seed: number): { int(n: number): number; pick<T>(xs: readonly T[]): T; chance(p: number): boolean } {
  let a = seed >>> 0;
  const next = (): number => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  return {
    int: (n) => Math.floor(next() * n),
    pick: (xs) => xs[Math.floor(next() * xs.length)] as (typeof xs)[number],
    chance: (p) => next() < p,
  };
}

// ---- inputs from the proto's fixtures

const ROW: ProjectRow = (() => {
  for (const e of core_ev) if (e.ev === "projects") for (const r of e.projects) if (r.cards !== undefined && r.cards.length > 0) return r;
  throw new Error("rust/proto/fixtures has no projects row with a card");
})();
const CARD: Card = ROW.cards?.[0] as Card;
const PROJECTS_EV = (() => {
  const e = core_ev.find((x) => x.ev === "projects");
  if (e?.ev !== "projects") throw new Error("no projects fixture");
  return e;
})();

const HOME = "bise-home-00000000";
const ACME = "acme-1a2b3c4d";
const BETA = "beta-5e6f7a8b";
/** the same small ids in two projects (a key without its project mixes them up) */
const IDS = [1, 2, 3, 4];
/** a burst past the 50 kept */
const BURST = Array.from({ length: 60 }, (_, i) => 100 + i);
const JUMPS = [0, 1, 1_000, 59_999, 60_000, 60_001, 90_000];

const card = (project: string, id: number, rank: number, since: number, act: OptAct | null): Card => ({
  ...CARD,
  kind: "question",
  project,
  id,
  rank,
  since_ms: since,
  question: `q${String(id)}`,
  options: [{ n: 1, label: "one" }, { n: 2, label: "two", ...(act === null ? {} : { act }) }],
});
/** the acts a hub's row may give an option (rows::opt_act): a link, an artifact (listed or not), both, a sign-in */
const ACTS: readonly (OptAct | null)[] = [
  null,
  { act: "open", url: "https://github.com/x/pull/1" },
  { act: "open", artifact: "a1", at: "m-1" },
  { act: "open", artifact: "a1", url: "http://127.0.0.1:1/p/a1#m-1", at: "m-1" },
  { act: "open", artifact: "gone", url: "http://127.0.0.1:1/p/gone" },
  { act: "sign_in" },
  { act: "open" },
];
/** a project's artifacts: none, a1 shown in the window, or a1 that only his browser opens */
const ARTS: readonly Artifact[][] = [
  [],
  [{ id: "a1", kind: "page", title: "a1", agent: "main", url: "http://127.0.0.1:1/p/a1", version: 1, at_ms: 1, new: false }],
  [{ id: "a1", kind: "pr", title: "a1", agent: "main", url: "https://github.com/x/pull/2", version: 1, at_ms: 1, new: false }],
];
const answerOf = (project: string, card: number): CardAnswer => ({ agent: "main", question: `q${String(card)}`, said: "1", notice: `answered ${project} ${String(card)}` });

type Row = { project: string; cards: readonly Card[] | undefined; home: boolean };

/** One run's inputs. */
function inputsOf(seed: number, n: number): Input[] {
  const r = rng(seed);
  let at = 1_791_100_000_000;
  const out: Input[] = [];
  const listed = new Map<string, Set<number>>([[HOME, new Set([1])], [ACME, new Set(IDS)], [BETA, new Set(IDS)]]);
  const burst = r.chance(0.2);
  if (burst) for (const id of BURST) listed.get(BETA)?.add(id);
  const ranks = new Map<string, number>();
  const acts = new Map<string, OptAct | null>();
  const actOf = (p: string, id: number): OptAct | null => {
    const k = `${p}-${String(id)}`;
    if (!acts.has(k)) acts.set(k, r.pick(ACTS));
    return acts.get(k) ?? null;
  };
  const rankOf = (p: string, id: number): number => {
    const k = `${p}-${String(id)}`;
    if (!ranks.has(k)) ranks.set(k, r.int(7));
    return ranks.get(k) as number;
  };
  const rowOf = (project: string): Row | null => {
    // not loaded: no row at all, or a row without its cards field (a reload, a hub starting)
    if (r.chance(0.1)) return null;
    if (r.chance(0.1)) return { project, cards: undefined, home: project === HOME };
    const ids = [...(listed.get(project) ?? [])];
    return { project, cards: ids.map((id) => card(project, id, rankOf(project, id), 1_000 + id, actOf(project, id))), home: project === HOME };
  };
  const projectsEv = (): Input => {
    const rows = [HOME, ACME, BETA].flatMap((p) => {
      const row = rowOf(p);
      if (row === null) return [];
      const { cards, ...rest } = row;
      return [{ ...ROW, ...rest, name: p.split("-")[0] as string, waits: cards?.length ?? 0, ...(cards === undefined ? { cards: undefined } : { cards: [...cards] }) }];
    });
    return { at_ms: at, in: "ev", ev: { ...PROJECTS_EV, projects: rows.map((x) => { const y = { ...x }; if (y.cards === undefined) delete (y as { cards?: unknown }).cards; return y; }) } };
  };
  const anyCard = (): [string, number] => {
    const p = r.pick([HOME, ACME, BETA, ACME, BETA]);
    const ids = p === BETA && burst && r.chance(0.3) ? BURST : p === HOME ? [1] : IDS;
    return [p, r.pick(ids)];
  };
  const fromOf = (): AnswerFrom | undefined =>
    r.pick<AnswerFrom | undefined>([undefined, { at: null, inbox: true }, { at: null }, { at: { project: HOME, agent: "main" } }, { at: { project: ACME, agent: "main" } }, { at: { project: BETA, agent: "perf" } }]);
  out.push(projectsEv());
  for (let i = 0; i < n; i += 1) {
    at += r.pick(JUMPS);
    const k = r.int(100);
    if (burst && i === 3) {
      // the burst: every one of its cards answered, in a row
      for (const id of BURST) out.push({ at_ms: at, in: "do", cid: 0, intent: { i: "answer", project: BETA, card: id, reply: "1" } });
      continue;
    }
    const [p, id] = anyCard();
    if (k < 22) {
      // the hub's rows: sometimes it dropped a card, sometimes a new one opened
      const d = r.int(10);
      if (d < 3) listed.get(p)?.delete(id);
      else if (d < 5) listed.get(p)?.add(id);
      out.push(projectsEv());
    } else if (k < 42) {
      const from = fromOf();
      out.push({ at_ms: at, in: "do", cid: 0, intent: { i: "answer", project: p, card: id, reply: r.pick(["1", "2", "the cold one"]), ...(r.chance(0.2) ? { parts: [{ kind: "image" as const, name: "[Image #1]", source: "/tmp/a.png" }] } : {}), ...(from === undefined ? {} : { from }) } });
    } else if (k < 48) {
      // his option n: an act the row gives it opens (nothing sent), else his answer
      const from = fromOf();
      out.push({ at_ms: at, in: "do", cid: 0, intent: { i: "pick", project: p, card: id, n: r.pick([1, 2, 2]), ...(from === undefined ? {} : { from }) } });
    } else if (k < 50) {
      out.push({ at_ms: at, in: "ev", ev: { ev: "artifacts", project: p, items: r.pick(ARTS) } });
    } else if (k < 54) out.push({ at_ms: at, in: "do", cid: 0, intent: { i: "close", project: p, card: id } });
    else if (k < 62) out.push({ at_ms: at, in: "do", cid: 0, intent: { i: "retry_card", project: p, card: id } });
    else if (k < 82) {
      // the hub took it: sometimes its rows dropped the card first, sometimes it kept it open
      if (r.chance(0.4)) {
        listed.get(p)?.delete(id);
        out.push(projectsEv());
      }
      out.push({ at_ms: at, in: "win", ev: { ev: "card_answered", project: p, card: id, answer: r.chance(0.8) ? answerOf(p, id) : null } });
      if (r.chance(0.3)) out.push(projectsEv());
    } else out.push({ at_ms: at, in: "win", ev: { ev: "card_refused", project: p, card: id, text: r.pick(["the card is gone", "card 3 isn't open"]) } });
  }
  return out;
}

// ---- the spec model

type Attempt = { kind: "close" } | { kind: "answer"; reply: string; parts?: readonly Part[]; from?: AnswerFrom };
interface Model {
  /** in flight: key -> when he acted, insertion order = oldest first */
  flight: Map<string, number>;
  attempts: Map<string, Attempt>;
  refused: Map<string, { text: string; attempt?: Attempt }>;
  answering: Map<string, AnswerFrom>;
  answersFrom: Map<string, AnswerFrom["at"] | "inbox">;
  jumps: { project: string; agent: string; card: number; seq: number }[];
}
const key = (p: string, c: number): string => `${p}-${String(c)}`;
const cap = <V>(m: Map<string, V>, n: number): void => {
  while (m.size > n) m.delete(m.keys().next().value as string);
};
const reAdd = <V>(m: Map<string, V>, k: string, v: V): void => {
  m.delete(k);
  m.set(k, v);
};
/** the loaded rows' open cards, by project; a project with no row or no cards field is not there */
const loadedOf = (rows: readonly ProjectRow[]): Map<string, Set<number>> =>
  new Map(rows.flatMap((x) => (x.cards === undefined ? [] : [[x.project, new Set(x.cards.map((c) => c.id))] as const])));

function modelStep(m: Model, f: HubFacts, input: Input): CoreCmd[] {
  const cmds: CoreCmd[] = [];
  const act = (p: string, c: number, at: number, a: Attempt): void => {
    reAdd(m.flight, key(p, c), at);
    cap(m.flight, 50);
    m.refused.delete(key(p, c));
    m.attempts.set(key(p, c), a);
  };
  const answer = (p: string, c: number, reply: string, parts: readonly Part[] | undefined, from: AnswerFrom | undefined, at: number): void => {
    if (from !== undefined) m.answering.set(`${p}/${String(c)}`, from);
    act(p, c, at, { kind: "answer", reply, ...(parts === undefined ? {} : { parts }), ...(from === undefined ? {} : { from }) });
    cmds.push({ cmd: "answer", project: p, card: c, reply, ...(parts !== undefined && parts.length > 0 ? { parts: [...parts] } : {}) });
  };
  const close = (p: string, c: number, at: number): void => {
    act(p, c, at, { kind: "close" });
    cmds.push({ cmd: "close", project: p, card: c });
  };
  if (input.in === "ev") {
    if (input.ev.ev !== "projects") return cmds;
    const loaded = loadedOf(f.rows);
    for (const [k, t] of [...m.flight]) {
      const [p, c] = [k.slice(0, k.lastIndexOf("-")), Number(k.slice(k.lastIndexOf("-") + 1))];
      const open = loaded.get(p);
      if (open === undefined) continue;
      if (!open.has(c) || input.at_ms - t >= 60_000) m.flight.delete(k);
    }
    for (const k of [...m.refused.keys()]) {
      const [p, c] = [k.slice(0, k.lastIndexOf("-")), Number(k.slice(k.lastIndexOf("-") + 1))];
      const open = loaded.get(p);
      if (open !== undefined && !open.has(c)) m.refused.delete(k);
    }
    for (const k of [...m.attempts.keys()]) if (!m.flight.has(k)) m.attempts.delete(k);
    return cmds;
  }
  if (input.in === "win" && (input.ev.ev === "card_answered" || input.ev.ev === "card_refused")) {
    const { project: p, card: c } = input.ev;
    const k = key(p, c);
    const from = m.answering.get(`${p}/${String(c)}`);
    m.answering.delete(`${p}/${String(c)}`);
    if (input.ev.ev === "card_refused") {
      m.flight.delete(k);
      const attempt = m.attempts.get(k);
      reAdd(m.refused, k, { text: input.ev.text, ...(attempt === undefined ? {} : { attempt }) });
      cap(m.refused, 50);
      m.attempts.delete(k);
      return cmds;
    }
    m.refused.delete(k);
    m.attempts.delete(k);
    if (f.rows.find((x) => x.project === p)?.cards?.some((x) => x.id === c) === true) m.flight.delete(k);
    if (from === undefined) return cmds;
    const where = from.inbox === true ? "inbox" : from.at;
    if (input.ev.answer !== null && where !== null) {
      reAdd(m.answersFrom, `${p}/${String(c)}`, where);
      cap(m.answersFrom, 200);
    }
    const home = f.rows.find((x) => x.home)?.project ?? "";
    if (input.ev.answer !== null && from.inbox !== true && from.at !== null && p !== home && from.at.project !== p)
      m.jumps.push({ project: p, agent: "main", card: c, seq: m.jumps.length + 1 });
    return cmds;
  }
  if (input.in !== "do") return cmds;
  const it = input.intent;
  if (it.i === "answer") answer(it.project, it.card, it.reply, it.parts, it.from, input.at_ms);
  else if (it.i === "close") close(it.project, it.card, input.at_ms);
  else if (it.i === "pick") {
    // the act the input's own row gives option n; none: his answer, its digit
    const act = f.rows.find((r) => r.project === it.project)?.cards?.find((c) => c.id === it.card)?.options.find((o) => o.n === it.n)?.act ?? null;
    if (act === null) answer(it.project, it.card, String(it.n), undefined, it.from, input.at_ms);
    else if (act.act === "sign_in") cmds.push({ cmd: "sign_in", id: "chatgpt" });
  } else if (it.i === "retry_card") {
    const a = m.refused.get(key(it.project, it.card))?.attempt;
    if (a?.kind === "close") close(it.project, it.card, input.at_ms);
    else if (a?.kind === "answer") answer(it.project, it.card, a.reply, a.parts, a.from, input.at_ms);
  }
  return cmds;
}

/**
 * What a pick opens, in the spec's words: a sign-in is a command; a link goes to his browser; an
 * artifact the project lists opens beside the thread (at its block), unless only his browser can
 * show it (a PR): then its own url; one not listed opens by its url, else still
 * its view (which says it's gone); nothing at all for an act with neither.
 */
function modelEffects(f: HubFacts, input: Input): Effect[] {
  if (input.in !== "do") return [];
  const it = input.intent;
  if (it.i !== "pick") return [];
  const act = f.rows.find((r) => r.project === it.project)?.cards?.find((c) => c.id === it.card)?.options.find((o) => o.n === it.n)?.act ?? null;
  if (act === null || act.act === "sign_in" || act.act === "unknown") return [];
  // a branch's guided review before its artifact (code-viewer-width)
  if (act.review?.kind === "branch") return [{ out: "nav", to: { project: it.project, branch: act.review.name } }];
  const arts = f.artifacts[it.project] ?? [];
  const id = act.artifact ?? null;
  const url = act.url ?? null;
  const a = id === null ? undefined : arts.find((x) => x.id === id);
  if (id !== null && (a !== undefined || url === null)) {
    if (a !== undefined && a.kind === "pr") return [{ out: "fx", fx: { fx: "openExternal", args: [a.url] } }];
    return [{ out: "nav", to: { project: it.project, artifact: id, at: act.at ?? null } }];
  }
  return url === null ? [] : [{ out: "fx", fx: { fx: "openExternal", args: [url] } }];
}

// ---- the laws

const cmdsOf = (outs: readonly Output[]): CoreCmd[] => outs.flatMap((o) => (o.out === "cmd" ? [o.cmd] : []));
const ids = (cs: readonly Pick<Card, "project" | "id">[]): string[] => cs.map((c) => key(c.project, c.id));
const sortedBy = (cs: readonly Card[]): boolean =>
  cs.every((c, i) => {
    const b = cs[i + 1];
    if (b === undefined) return true;
    const ra = c.rank ?? 6;
    const rb = b.rank ?? 6;
    return ra < rb || (ra === rb && (c.since_ms < b.since_ms || (c.since_ms === b.since_ms && (c.project < b.project || (c.project === b.project && c.id <= b.id)))));
  });

const SEEDS = process.env["SEED"] !== undefined ? [Number(process.env["SEED"])] : Array.from({ length: 200 }, (_, i) => i + 1);
// the 200 seeds in 10 tests of 20: each ~3 s alone, so each fits its 60 s under a tip gate's load
// (one test of 200 took 30 s alone and 68 s at load 20-60 on 41dd76670). Same seeds, same laws.
const CHUNK = 20;
const CHUNKS = Array.from({ length: Math.ceil(SEEDS.length / CHUNK) }, (_, i) => SEEDS.slice(i * CHUNK, (i + 1) * CHUNK));

describe("the cards slice's law", () => {
  for (const chunk of CHUNKS) it(`over seeded runs ${String(chunk[0])}-${String(chunk[chunk.length - 1])}: the inbox is the hub's open cards less his acts in flight; one gesture, one command; a reload keeps his; the hub's rank orders it; nothing changed is the same state`, () => {
    for (const seed of chunk) {
      let s: CardsState = cardsInit(null);
      let f: HubFacts = NO_FACTS;
      const m: Model = { flight: new Map(), attempts: new Map(), refused: new Map(), answering: new Map(), answersFrom: new Map(), jumps: [] };
      const keep = new Map<string, unknown>();
      inputsOf(seed, 80).forEach((input, step) => {
        const at = `seed ${String(seed)} step ${String(step)} (${input.in === "do" ? input.intent.i : input.ev.ev})`;
        if (input.in === "ev") f = factsOf(f, input.ev);
        const before = JSON.stringify(s);
        const [next, outs] = stepCards(s, f, input);
        // 1. pure: the state it was given is untouched, and the same input gives the same answer
        expect(JSON.stringify(s), at).toBe(before);
        expect(JSON.stringify(stepCards(s, f, input)), at).toBe(JSON.stringify([next, outs]));
        // 6. an event that changes nothing: the same state back, nothing out (his gesture and a
        //    refusal are always written again, as today's store did: news even when equal)
        if (JSON.stringify(next) === before && input.in !== "do" && !(input.in === "win" && input.ev.ev === "card_refused")) {
          expect(next, at).toBe(s);
          expect(outs, at).toEqual([]);
        }
        // 3. one gesture, one command (the same words a retry sends again); nothing else sends
        const want = modelStep(m, f, input);
        expect(cmdsOf(outs), at).toEqual(want);
        if (input.in !== "do") expect(cmdsOf(outs), at).toEqual([]);
        // 5'. the act comes from the input's own row (rows::opt_act), never made in step: what a
        //     pick opens follows it, and nothing but a pick or an open opens anything
        expect(outs.filter((o) => o.out === "nav" || o.out === "fx"), at).toEqual(modelEffects(f, input));
        // 4. what's kept is the state's own list, and a reload reads back the same
        for (const o of outs) if (o.out === "keep") keep.set(o.key, o.value);
        for (const o of outs) if (o.out === "keep") expect(o.value, at).toBe(o.key === "acted" ? next.acted : o.key === "refused" ? next.refused : next.answersFrom);
        const back = cardsInit((k) => (keep.has(k) ? JSON.parse(JSON.stringify(keep.get(k))) : undefined));
        expect(JSON.stringify([back.acted, back.refused, back.answersFrom]), at).toBe(JSON.stringify([next.acted, next.refused, next.answersFrom]));
        s = next;
        // 2. the hub owns open and closed: the inbox lists the rows' open cards less his own acts
        //    in flight, never one the rows don't list, never one he acted on that's still on its way
        const shown = inboxView(s, f);
        const open = f.rows.flatMap((r) => (r.cards ?? []).map((c) => key(r.project, c.id)));
        expect(new Set(ids(shown)), at).toEqual(new Set(open.filter((k) => !m.flight.has(k))));
        // each item says the refusal the model holds for it, or none
        for (const c of shown) expect(c.refused?.text ?? null, at).toBe(m.refused.get(key(c.project, c.id))?.text ?? null);
        // a refusal whose card the hub withdrew (its loaded row no longer lists it) is gone after
        // those rows: no item, kept or listed, can say it (architect m_29873)
        if (input.in === "ev" && input.ev.ev === "projects") {
          const loaded = loadedOf(f.rows);
          for (const x of s.refused) expect(loaded.get(x.project)?.has(x.card) ?? true, `${at}: ${x.key}`).toBe(true);
        }
        // the refusals: their words and what he sent, on the cards the model says
        expect(new Map(s.refused.map((x) => [x.key, { text: x.text, ...(x.attempt === undefined ? {} : { attempt: x.attempt }) }])), at).toEqual(m.refused);
        expect(new Map(s.answersFrom.map((x) => [x.key, x.from])), at).toEqual(m.answersFrom);
        expect(s.jump, at).toEqual(m.jumps[m.jumps.length - 1] ?? null);
        // 5. the hub's rules come from the event: its rank orders the list, step computes none
        expect(sortedBy(shown), at).toBe(true);
      });
    }
  }, 60_000);
});
