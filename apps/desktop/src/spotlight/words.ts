// The spotlight's words, as ambient signed them for the old web capsule (its words.js `spot`,
// m_9164, m_9196). Pure.

export const W = {
  ph: 'ask bise, or find an agent',
  to: 'to bise',
  holdFn: 'hold fn to talk',
  waiting: 'waiting for you',
  atWork: 'at work',
  envs: 'projects',
  found: 'found',
  /** a waiting row's own key, like the other rows' (ambient-pm's try gate): ⏎ answers it */
  answerKey: '⏎ answer',
  /** the key on an agent's or a project's active row (designer m_25150) */
  openKey: '⏎ open',
  /** a row the keyboard is on under a message (Gabriel's v4): his words go as its draft, or to it */
  openWithKey: '⏎ open with your words   ⌘⏎ send to it',
  startKey: '⏎ start',
  openInBise: 'open in bise',
  close: 'close',
  inFront: (what: string): string => (what ? `in front: ${what}` : 'in front'),
  /** the count first: a long path is the part that gets cut (designer N4) */
  envSub: (branch: string, n: number): string => `${n} agent${n === 1 ? '' : 's'} · ${branch}`,
  // the key line (Gabriel's v4, spotlight-composer#choix): what ⏎ does says which way it goes
  keysEmpty: '⏎ open   ↓ the list   esc close',
  keysSearch: (name: string): string => `⏎ open ${name}   ↓ the list   esc close`,
  keysMessage: '⏎ send to bise   ⇧⏎ new line   esc close, draft kept',
  keysList: '↑ on the first row: the text   esc the text',
  keysMenu: '⏎ pick   ⇥ complete   ↑↓ move   esc close the menu',
  answerMore: 'the rest in bise   ⏎ open',
  // spot-shot: the window he was on rides his ask as a chip (the talk's 'no screenshot' words)
  shotDrop: "don't send this window",
  // the ❝ chip (Gabriel's v4): his selection read at fn space, sent first with his message
  quoteDrop: "don't send this selection",
  // his switch wants the still but Screen Recording is off: the chip says it and opens the setting
  shotOff: 'no screenshot · Screen Recording is off',
  shotOffTitle: 'open System Settings to let bise see the window you ask about',
  // his ask, before bise's first words (R18): it went, the answer comes here
  asked: 'asked bise · the answer shows here',
  // bise sent his ask to another project's thread: its answer is there, not here
  wentTo: (name: string): string => `sent to ${name} · its answer is in ${name}'s thread`,
  // the routing hold (bar ⛔3, lead m_9855): bise picked the project, he has 2 s to change it;
  // the same words as the thread's hold (lead m_11564, e298b066: main is 'bise')
  routeTo: (name: string, s: number): string => (s > 0 ? `→ bise · ${name} · ${s} s to change` : `→ bise · ${name} · sending`),
  keepWithBise: 'keep it with bise',
  anotherProject: 'another project',
  keysRoute: 'esc keep it with bise   ↑↓ ⏎ another project',
  // A.1: '/new …' or 'start an agent: …' starts an agent in bise's home with that brief
  startAgent: 'start an agent',
  keysNew: '⏎ start it   esc close',
  // the thread pane (page spotlight-reponse, Gabriel's pick B): a follow-up opens it on the right
  paneHead: 'bise · this thread',
  paneOpen: 'open in bise',
  paneOpenTip: 'open this thread in bise · ⌘⏎',
  paneEndTip: 'end this thread · ⌘W',
  panePh: 'follow up',
  paneKeys: [
    ['⏎', 'send'],
    ['tab', 'back to search'],
    ['⌘W', 'end'],
  ] as const,
} as const;

/**
 * The brief of '/new <brief>' or 'start an agent: <brief>' (the colon optional, any case), else
 * undefined: the TUI's /new with its brief, typed in the spotlight (bar A.1). Pure.
 */
export function newBrief(q: string): string | undefined {
  const m = /^\s*(?:\/new|start an agent\s*:?)(?:\s+|$)([\s\S]*)$/iu.exec(q);
  if (m === null) return undefined;
  const brief = (m[1] ?? '').trim();
  return brief === '' ? undefined : brief;
}

/** One line of text: tags out, whitespace runs as one space, cut at a word past `max` with `…`. */
export function oneLine(text: string, max = 90): string {
  const t = text.replace(/<\/?[a-z][\w-]*(?:\s[^<>]*)?\/?>/giu, ' ').replace(/\s+/gu, ' ').trim();
  if (t.length <= max) return t;
  const cut = t.slice(0, max - 1).replace(/\s+\S*$/u, '').replace(/[\s·,;:]+$/u, '');
  return `${cut === '' ? t.slice(0, max - 1) : cut}…`;
}

/**
 * An approval's row (ambient m_9164): its first line, then what the second adds without the names
 * the first already said ('tryme is ready to try · 1 commit · +3 −0'), cut with … past max.
 */
export function cardLine(text: string, max = 64): string {
  const lines = text.split('\n').map((l) => oneLine(l, 400)).filter((l) => l !== '');
  const [head, next] = lines;
  if (head === undefined) return '';
  const toks = (l: string): string[] => l.split(/\s+/u).filter((x) => x !== '');
  const key = (t: string): string => t.toLowerCase().replace(/[^\p{L}\p{N}_./-]/gu, '');
  const named = new Set(toks(head).filter((t, i) => i === 0 || /[-_./\d]/u.test(t)).map(key).filter((k) => k !== ''));
  const out: string[] = [];
  for (const t of toks(next ?? '')) {
    if (named.has(key(t))) {
      if (/^(on|in|at|of|for|to|from)$/iu.test(out.at(-1) ?? '')) out.pop();
      continue;
    }
    out.push(t);
  }
  const second = out.join(' ').replace(/·(\s*·)+/gu, '·').replace(/^[\s·]+|[\s·]+$/gu, '');
  const s = [head, second].filter((x) => x !== '').join(' · ');
  return s.length > max ? `${s.slice(0, max - 1).replace(/\s+\S*$/u, '').replace(/[\s·]+$/u, '')}…` : s;
}

/** A path as he reads it: his home as ~, a long one cut in the middle keeping the last folders. */
export function shortPath(p: string, max = 32): string {
  const s = p.replace(/^\/(?:Users|home)\/[^/]+(?=\/|$)/u, '~');
  if (s.length <= max) return s;
  const parts = s.split('/');
  const head = parts[0] ?? '';
  let tail = parts.at(-1) ?? '';
  for (let i = parts.length - 2; i > 0 && `${head}/…/${parts[i] ?? ''}/${tail}`.length <= max; i -= 1) tail = `${parts[i] ?? ''}/${tail}`;
  return `${head}/…/${tail}`;
}

/** The host of a URL, or ''. */
export const hostOf = (url: string): string => /^[a-z]+:\/\/([^/:]+)/iu.exec(url)?.[1] ?? '';

/** Words of a text, lowercase, letters and digits (and _ -). */
export const words = (s: string): string[] => s.toLowerCase().split(/[^\p{L}\p{N}_-]+/u).filter((w) => w !== '');
