// The thread's composer (amb-web's Composer with this thread's target, sources, drafts, answers
// and slash commands), also mounted under an open artifact and in the floating composer
// (composer/float, `floating`: no strip, '+' alone); its head the context strip (ComposerTab.tsx), its notice the hub's word on his
// last line; and its pure helpers (about-an-artifact words, the docked card's target, the notice).
// Split out of ThreadScreen.tsx (the file cap). Owner: nm-composer.
import { memo, useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { ANCHOR_EVENT, type AnchorAct } from '@/artifacts/useFrameNote';
import { load, save } from '@/composer/drafts';
import { shownName } from '@/agents/model';
import { Composer, flashWords, useApprovalsFlash, useComposerSources } from '@/composer';
import type { ComposerActions, Notice } from '@/composer';
import type { Outgoing } from '@/composer/message';
import { threadKey, useApprovals, useCards, useCommands, useCoreEvent, useFiles, useNotice, useSkills, useStore, type HubLine } from '@/store';
import { useRouter } from '@/app/router';
import { pickCommands, slashLine, slashOf, windowRun } from './commands';
import { HELP_EVENT } from '@/app/keys';
import { keptStorage } from '@/app/keep';
import { cardDraftKey, leaveClosed, openInbox } from '@/inbox';
import { emptyEnterAct } from './head';
import { useComposerHeld } from './foldHint';
import { computerRowOf, computerState } from './ComputerLine';
import type { Agent } from '@/proto';
import { ComposerTab } from './ComposerTab';
import { useThreadQueue } from './ThreadQueue';
import { AttachTool, ModelTool, ModeTool } from './ComposerTools';
import { pinkState, type TabWords } from './tab';
import { leadAgent, leadMessage } from './lead';
import { composerWords } from './head';
import { ABOUT_HEAD } from '@/artifacts/maker';
import { otherTip } from '@/app/tipwords';
import { aboutParts, aboutText } from './about';
import { composer } from '../proto/words.gen';
import { useHomeShows } from '@/composer/home/Home';
import { noteComposerFocus, talkFocusHandlers } from '@/composer/talking';

/**
 * The thread's composer, also mounted under an open artifact (amb-kit's src/artifacts/Open.tsx,
 * desktop-screens v6) with `about`: its own drafts, and the message names the artifact.
 */
export const ThreadComposer = memo(function ThreadComposer({
  project,
  agent,
  busy,
  tab,
  tabEnd,
  me,
  placeholder,
  placeholderTail,
  answer,
  onStop,
  onEscape,
  onReady,
  onSent,
  about,
  aboutPink,
  voiceTyping = false,
  lead,
  mark,
  hubNotes = false,
  placeOnly = false,
  floating = false,
  onCanSend,
}: {
  project: string;
  agent: string;
  busy: boolean;
  /** the context strip's words (the box's head): where it works, the context; bare, the label */
  tab: TabWords;
  /** after the label's first part: the addressee's working gust (LabelMark), else none */
  mark?: ReactNode;
  /** the hub's plain lines show on the key line too (a screen with no thread: no HubNoteRow says them) */
  hubNotes?: boolean;
  /** the empty home: the strip's place is 'new worktree' (main starts its agents in one) */
  placeOnly?: boolean;
  /** the floating composer's (composer/float): no strip (its head says whom it writes to), '+'
   *  alone in the tools (no mode, no model) */
  floating?: boolean;
  /** the field holds something to send (the floating composer stays unfolded while it does) */
  onCanSend?: (can: boolean) => void;
  /** the strip's right instead of the context: 'esc to bise instead' while it answers, 'tab back to voice' */
  tabEnd?: string | undefined;
  /** the addressee's row: its model menu in the box's row (none: no menu) */
  me?: Agent | undefined;
  /** the field's words when empty (the composer's own 'write to …' when none) */
  placeholder: string | undefined;
  /** after the placeholder, dropped first when the line doesn't fit: a sub-agent's 'esc back to bise' */
  placeholderTail?: string | undefined;
  /** the open card his words answer (the inbox block's, any project's), else none */
  answer: CardAt | null;
  onStop: () => void;
  /** an esc the composer's lists didn't take (R18): the screen's (route_cancel, stop); false: nothing to do, the window may go up (esc-up.ts) */
  onEscape?: () => boolean;
  onReady: (a: ComposerActions) => void;
  /** his send went (the demo tour's step) */
  onSent: () => void;
  /** the artifact his words are about (an open artifact's composer): sent as `about [title](url): …` */
  about?: { id: string; title: string; url: string };
  /** the open artifact's words go to a live sub-agent (art-target): the pink, with this tip, while no item is open */
  aboutPink?: string | undefined;
  /** voice mode while he types (designer's voice-window (1)): no @name lead, no pink of its own */
  voiceTyping?: boolean;
  /** in the box at its left (voice mode's grey pearl while he types) */
  lead?: ReactNode;
}) {
  const store = useStore();
  const router = useRouter();
  // main's empty thread (composer/home): the strip says 'new worktree', the field the home's words
  const home = useHomeShows(threadKey(project, agent), agent) && !floating && about === undefined && answer === null && !voiceTyping;
  // TP-R6 (amb-web's composer/approvals.ts): ⇧⇥ switches the hub's approvals mode yolo ↔ auto like
  // the TUI; for 3 s after a switch the label line's right says it (the TUI's key bar flash)
  const approvals = useApprovals(project);
  const flash = useApprovalsFlash(approvals) && approvals !== null ? flashWords(approvals.mode, approvals.checker, approvals.env) : null;
  const end: ReactNode =
    flash === null ? tabEnd : (
      <span data-testid="composer.flash">
        <span className="flash-word">{flash.word}</span>
        {flash.rest}
      </span>
    );
  const onShiftTab = useCallback(() => {
    store.command({ cmd: 'approvals', project, mode: 'toggle' });
  }, [store, project]);
  // the composer's lists from the core (amb-core's C/A/B 40f14a12): the TUI's
  // commands in its order (window ones ready when their screen exists, the
  // hub's when they run typed), the workspace's $ skills, its @ files ranked
  // by files.rs (a newer query replaces an older one: [] for it)
  const core = useCommands();
  const skills = useSkills(project);
  // while the workspace's first walk runs, an '@' answer is partial and the
  // fuller one comes later (useFiles): when it does, the files source changes
  // once, so an open '@' list asks again and shows it
  const filesSettled = useFiles(project)?.partial !== true;
  const extra = useMemo(
    () => ({
      // a new files source once the walk settles: the composer's '@' asks again
      settled: filesSettled,
      commands: pickCommands(core),
      skills,
      files: (q: string) => store.files(project, q).then((a) => (a === null ? [] : [...a.items, ...(a.folder ? [{ path: a.folder.path, dir: true }] : [])])),
      // ctrl+r twice: voice mode with this agent (amb-home's src/voicemode)
      voice: () => {
        store.voiceMode(project, agent, true);
      },
    }),
    [core, skills, store, project, agent, filesSettled],
  );
  const sources = useComposerSources(project, extra, agent);
  // his words opening with `@<a live agent>` (at-start, lead.ts): that agent's pink while he types,
  // the send goes to its thread and the window opens it; never on an item, an artifact or voice
  const leads = answer === null && about === undefined && !voiceTyping;
  const live = useMemo(() => sources.agents.map((a) => a.name), [sources.agents]);
  const [leadTo, setLeadTo] = useState<string | null>(null);
  const onLead = useCallback(
    (t: string) => {
      setLeadTo(leadAgent(t, live, agent));
    },
    [live, agent],
  );
  const atTo = leads ? leadTo : null;
  // the hub's word on his answer, where he typed it (main m_21514, amb-core m_21541): a drafts card
  // says where his words went; a refusal says why and gives his words back (never a field that just
  // empties, whichever project's card it was)
  const asked = useRef<{ project: string; text: string } | null>(null);
  const actions = useRef<ComposerActions | null>(null);
  const [note, setNote] = useState<AnswerNote | null>(null);
  // his last line the hub's router runs ('/bime'): refused, it comes back in the empty field, all
  // selected, so one key replaces it or an arrow fixes it (designer m_29135); the key line says why
  const slashSent = useRef<string | null>(null);
  useCoreEvent(
    useCallback(
      (ev) => {
        const back = refusedSlashOf(ev, project);
        if (back === null) return;
        const line = slashSent.current;
        slashSent.current = null;
        if (back && line !== null && fieldEmpty()) actions.current?.fill(line);
      },
      [project],
    ),
  );
  useCoreEvent(
    useCallback((ev) => {
      const n = answerNoteOf(ev, asked.current);
      if (n === null) return;
      setNote(n);
      if (n.level === 'err' && asked.current !== null && fieldEmpty()) actions.current?.fill(asked.current.text);
      asked.current = null;
    }, []),
  );
  const ready = useCallback(
    (a: ComposerActions) => {
      actions.current = a;
      onReady(a);
    },
    [onReady],
  );
  // his queued messages, the hub's (issue 23): tab queues there, the drawer above the tab shows
  // them with send now / edit / delete, ↑ in the empty field edits the newest (ThreadQueue.ts)
  const queue = useThreadQueue(project, agent, actions);
  // the composer about an open artifact: a selection in its frame (artifacts/useFrameNote.ts)
  // comes as a numbered attachment at the caret, and the composer takes the focus for his words
  const aboutId = about?.id;
  useEffect(() => {
    if (aboutId === undefined) return;
    const on = (e: Event) => {
      const d = (e as CustomEvent<AnchorAct>).detail;
      if (d.project !== project || d.id !== aboutId) return;
      const a = actions.current;
      if (a === null) return;
      // his first typed character took the selection in the frame: it goes after the chip
      const { key, ...note } = d.note;
      a.anchor({ ...note, file: d.file });
      if (key !== undefined) a.insertText(key);
    };
    window.addEventListener(ANCHOR_EVENT, on);
    return () => {
      window.removeEventListener(ANCHOR_EVENT, on);
    };
  }, [project, aboutId]);
  const onCommand = useCallback(
    (name: string, args: string) => {
      const at = { project, agent };
      const w = windowRun(name, args, at);
      if (w !== undefined) {
        if ('go' in w) router.go(w.go);
        // nothing waits anywhere: say so where he typed it, never a silent nothing (ghost-badge)
        else if ('inbox' in w) {
          if (!openInbox()) setNote({ level: 'info', text: composer.nothing_waits });
        }
        else if ('app' in w) store.command(w.app);
        else if ('quit' in w) window.bise.quit();
        else window.dispatchEvent(new Event(HELP_EVENT));
        return;
      }
      // every other line he typed goes to the hub as typed: its router runs it (architect m_11011)
      const cmd = slashOf(name, args, at);
      slashSent.current = cmd.line;
      store.command(cmd);
    },
    [router, store, project, agent],
  );
  const onSend = useCallback(
    (out: Outgoing) => {
      // a '/word' line the composer's list doesn't know is still a command: the hub's router runs it
      // (never a line with typed parts: '/word [Image #1]' or a quoted '/word' is a message)
      // any send ends the key line's notice (read through a ref: the send never re-binds per notice)
      doneRef.current();
      const sl = out.parts === undefined ? slashLine(out.text) : null;
      if (sl !== null) {
        onCommand(sl.name, sl.args);
        return;
      }
      // his words open with `@<a live agent>` (at-start): they go to its thread, without the
      // `@name`, as if he typed them there (its row there, never one left here), and it opens
      const to = leads ? leadAgent(out.text, live, agent) : null;
      if (to !== null) {
        const msg = leadMessage(out, to);
        if (msg !== null) {
          const cu = computerRowOf(store.computerLive(), project, to);
          if (cu !== undefined && computerState(cu) === 'stopped') store.computer('hand_back', cu.key);
          // from: the hub leaves '→ sent to <to>' where he typed it (the trace, at-start sha 2)
          store.send({ project, agent: to, text: msg.text, mode: 'now', from: agent, ...(msg.parts === undefined ? {} : { parts: msg.parts }) });
        }
        setLeadTo(null);
        router.go({ screen: 'thread', project, agent: to });
        onSent();
        return;
      }
      // the open card's answer (any project's: the block lists them all): his words and the images
      // he attached, the hub's one /answer path (R41)
      // the card he answered was closed by the hub while he typed (card-draft-keep): his words go
      // to its asker as a plain message, then the block lets the card go
      // an open item: his free text answers it, refuses it or steers its agent, and it closes
      // (cardActs, his rule m_44704); the hub's word on an answer comes back here (answerNoteOf)
      if (answer !== null) {
        for (const a of cardActs(answer, out, { project, agent })) {
          if (a.act === 'send') store.send(a.send);
          else {
            if (a.do.i === 'answer') asked.current = { project: answer.project, text: out.text };
            store.do(a.do);
          }
        }
        if (answer.closed === true && answer.asker !== undefined) {
          sentClosed.current = true;
          leaveClosed();
        }
        onSent();
        return;
      }
      // ⏎ while a turn runs steers it ('now', as the TUI); the images and
      // files he attached or dropped go with it as parts. Tab's queue: amb-web's
      const text = about === undefined ? out.text : aboutText(about, out.text);
      // S29/L19 W2: stopped until he writes: his message gives the wheel back first (read at send,
      // never subscribed: the rows moving must not re-render the composer)
      const cu = computerRowOf(store.computerLive(), project, agent);
      if (cu !== undefined && computerState(cu) === 'stopped') store.computer('hand_back', cu.key);
      // msg-parts: his quotes, pastes and images go typed, never templated by the window; an
      // artifact's question puts its 'about' lead in front of his words, as a text part too
      const parts = about === undefined || out.parts === undefined ? out.parts : aboutParts(about, out.parts);
      store.send({ project, agent, text, mode: 'now', ...(parts === undefined ? {} : { parts }) });
      onSent();
    },
    [store, router, project, agent, onSent, onCommand, answer, about, leads, live],
  );
  // his drafts, history and queue in the keep's file, never lost by a quit (keep.ts)
  // K2: ⏎ on an empty field does the TUI's act, if one waits (sign in again after expiry)
  const cards = useCards(project);
  const onEmptyEnter = useCallback(() => {
    // S29/L19 W2: he took the wheel: ⏎ on the empty field gives it back ('you took the wheel · ⏎ give it back')
    const cu = computerRowOf(store.computerLive(), project, agent);
    if (cu !== undefined && computerState(cu) === 'paused') {
      store.computer('hand_back', cu.key);
      return;
    }
    // his /restart's row says a newer app is staged: ⏎ restarts the app (amb-home, ambient-lead m_22833)
    const swap = document.querySelector<HTMLButtonElement>('[data-testid="confirm.notice.swap"]');
    if (swap !== null) {
      swap.click();
      return;
    }
    // the thread's last row, read at ⏎ (never subscribed: a new entry must not re-render the composer)
    const key = threadKey(project, agent);
    const lastPos = store.threadOrder(key).order.at(-1);
    const lastKind = lastPos === undefined ? undefined : store.entry(key, lastPos)?.kind;
    const act = emptyEnterAct(cards, agent, lastPos === undefined || lastKind === undefined ? undefined : { kind: lastKind, pos: lastPos });
    if (act === null) return;
    // his undelivered message, last in the thread: its own row sends it again (and says so); gone
    // once sent or dropped, so a second ⏎ does nothing, as the TUI's
    if (act.act === 'again') document.querySelector<HTMLButtonElement>(`[data-testid="thread.notdelivered.${String(act.pos)}.again"]`)?.click();
    else store.command(act.cmd);
  }, [cards, agent, store, project]);
  // the open card that takes words keeps his half answer apart, the thread's draft never moves
  // (amb-home 2b, amb-web's draftKey): the card's own key, the same wherever the block opens it
  const draftKey = answer === null ? undefined : cardDraftKey(answer);
  // he left a closed card without sending (esc, another item): its words come to this thread's
  // composer when it is empty, else on top of its history, with one line (designer m_24087). The
  // field already swapped (its effect, the child's, ran first) and kept them under the card's key.
  const closedKey = answer !== null && answer.closed === true ? draftKey : undefined;
  const wasClosed = useRef<{ key: string; asker: string } | null>(null);
  const sentClosed = useRef(false);
  useEffect(() => {
    const was = wasClosed.current;
    wasClosed.current = closedKey === undefined ? null : { key: closedKey, asker: answer?.asker ?? '' };
    if (was === null || was.key === closedKey) return;
    if (sentClosed.current) {
      sentClosed.current = false;
      return;
    }
    const entry = load(keptStorage, was.key).draft;
    if (entry === null) return;
    save(keptStorage, was.key, { draft: null, history: [] });
    if (actions.current?.putBack(entry) === 'history') setNote({ level: 'info', text: composer.closed_question({ agent: shownName(was.asker) }) });
  }, [closedKey, answer?.asker]);
  // the box's row: '+' always; the mode and the model only for a message (an answer has neither)
  const answering = answer !== null;
  // the one pink state (tab.ts pinkState): an open question, an open artifact's live maker, or an
  // agent that is not main (agent-target); an item open wins (an approval's plain line)
  const leadTip = atTo === null ? undefined : composer.lead_tip({ agent: shownName(atTo), here: shownName(agent) });
  const ps = pinkState({ answer, leadTip, aboutPink, about: about !== undefined, agent, voiceTyping, otherTip: (a) => otherTip(shownName(a)) });
  const pink = ps.pink;
  const pinkTip = ps.tip;
  const attach = useCallback((paths: readonly string[]) => {
    actions.current?.attach(paths);
  }, []);
  const tools = useMemo(
    () => (
      <>
        <AttachTool onPick={attach} />
        {!answering && !floating && <ModeTool project={project} />}
      </>
    ),
    [attach, answering, project, floating],
  );
  const toolsEnd = answering || floating || me === undefined ? undefined : <ModelTool project={project} a={me} />;
  // a modifier held alone: its keys in the composer's row, ⌃R on the mic (composer/heldkeys.ts)
  const held = useComposerHeld();
  // answering, the tab holds only 'answering <agent> · <question>' and the way back: its place and
  // context come back with a plain message (designer m_24517)
  // (the pink artifact's tab the same: '↳ to perf · about “…”' and 'esc to bise instead')
  // (its '· about “…”' shows whole or not at all, designer m_27021)
  // (an agent that is not main keeps its place and context: it is its own thread)
  const { bare, whole } = ps;
  // his words open with `@<agent>`: '↳ to architect' alone, and how to stay here on the right
  const label = atTo === null ? tab.label : `${ABOUT_HEAD}${composerWords(shownName(atTo)).label}`;
  const tabWords = useMemo(() => (bare ? { label, ...(whole ? { whole } : {}) } : tab), [bare, whole, tab, label]);
  const endWords = atTo === null || flash !== null ? end : composer.lead_end({ here: shownName(agent) });
  // the strip's addressee: its picker opens another agent's thread of this project (his m_47126);
  // the draft stays with the thread it was typed in (its own target). Under an open artifact the
  // artifact decides who: its name a label (designer m_47129)
  const aboutOpen = about !== undefined;
  const stripAgent = useMemo(() => ({ project, agent, ...(aboutOpen ? {} : { onPick: (a: string) => {
    router.go({ screen: 'thread', project, agent: a });
  } }) }), [project, agent, router, aboutOpen]);
  const tabEl = <ComposerTab words={tabWords} bare={bare || atTo !== null} to={toAccent(ps, atTo)} asks={ps.asks} who={stripAgent} at={me} mark={atTo === null ? mark : undefined} end={endWords ?? undefined} project={project} newPlace={placeOnly || home} testId="composer.ctx" />;
  // the one notice at the strip's right (noticeOf): it goes at his first key in the frame (esc
  // too), at any send or command, a note also after LINE_MS; never outlives his next message,
  // never moves the composer (designer m_29135)
  const hubLine = useNotice(project);
  const shown = useMemo(() => noticeOf(hubLine, note, hubNotes), [hubLine, note, hubNotes]);
  const lineDone = useCallback(() => {
    setNote(null);
    if (shown?.hub != null) store.dropNotice(project, shown.hub);
  }, [shown, store, project]);
  // an error has no timer: it says why his line is back in the field, until he acts (designer m_29135)
  useEffect(() => {
    if (shown === null || shown.line.level === 'err') return;
    const t = window.setTimeout(lineDone, LINE_MS);
    return () => {
      window.clearTimeout(t);
    };
  }, [shown, lineDone]);
  const doneRef = useRef(lineDone);
  useEffect(() => {
    doneRef.current = lineDone;
  });
  const field = <Composer line={shown?.line ?? null} onLineDone={lineDone} target={about === undefined ? `${project}/${agent}` : `${project}/${agent}/artifact/${about.id}`} {...(draftKey === undefined ? {} : { draftKey })} busy={busy} sources={sources} store={keptStorage} head={floating ? undefined : tabEl} answering={pink} answerTip={pinkTip} {...(ps.quiet ? { quiet: true } : {})} tools={tools} toolsEnd={toolsEnd} held={held} onShiftTab={onShiftTab} placeholder={placeholder} placeholderTail={placeholderTail} onEmptyEnter={onEmptyEnter} {...(onEscape === undefined ? {} : { onEscape })} onSend={onSend} onLead={onLead} leadName={atTo} {...(answering ? {} : { onQueue: queue.onQueue, onEditNewest: queue.onEditNewest, queue: queue.slot })} onCommand={onCommand} onStop={onStop} onReady={ready} lead={lead} {...(onCanSend === undefined ? {} : { onCanSend })} testId="composer" />;
  // its focus says whom he talks to (composer/talking.ts: nm-side's camera); the wrapper has no box
  const talk = useMemo(() => talkFocusHandlers({ project, agent }), [project, agent]);
  // gone while focused (a route change): no blur comes, so it lets go here
  useEffect(
    () => () => {
      noteComposerFocus(null, { project, agent });
    },
    [project, agent],
  );
  return (
    <div className="cmp-talk" onFocus={talk.onFocus} onBlur={talk.onBlur}>
      {field}
    </div>
  );
});

/** How long a notice stays when he does nothing (designer m_29135; an error has no timer). */
export const LINE_MS = 8000;

/**
 * The composer's one notice (his m_29126: a refused '/bime' stayed in a box under it for many
 * messages and pushed the composer up): the hub's refused line first (his words to fix), then the
 * composer's own note, then the hub's plain line where no thread row shows it (`hubNotes`). Pure.
 */
export function noticeOf(hub: HubLine | null, note: AnswerNote | null, hubNotes: boolean): { line: Notice; hub: HubLine | null } | null {
  if (hub !== null && hub.level === 'error') return { line: { level: 'err', text: hub.text, testId: 'confirm.notice' }, hub };
  if (note !== null) return { line: { level: note.level, text: note.text, testId: 'composer.answer.note' }, hub: null };
  if (hubNotes && hub !== null && hub.back !== true) return { line: { level: 'info', text: hub.text, testId: 'confirm.notice' }, hub };
  return null;
}

/** The hub's word on an answer this composer sent: its notice (where his words went) or its refusal. */
export interface AnswerNote {
  readonly level: 'info' | 'err';
  readonly text: string;
}

/** A core event as the note on his last answer, if it is one: the same project, cmd 'answer'. Pure. */
export function answerNoteOf(ev: unknown, asked: { project: string } | null): AnswerNote | null {
  if (asked === null || typeof ev !== 'object' || ev === null) return null;
  const e = ev as { ev?: unknown; cmd?: unknown; project?: unknown; text?: unknown };
  if (e.cmd !== 'answer' || e.project !== asked.project || typeof e.text !== 'string') return null;
  if (e.ev === 'notice') return { level: 'info', text: e.text };
  if (e.ev === 'error') return { level: 'err', text: e.text };
  return null;
}

/** A core event as the hub's word on a slash line of this project: true refused, false taken (its
 *  notice), null another event. Pure. */
export function refusedSlashOf(ev: unknown, project: string): boolean | null {
  if (typeof ev !== 'object' || ev === null) return null;
  const e = ev as { ev?: unknown; cmd?: unknown; project?: unknown };
  if (e.cmd !== 'slash' || e.project !== project) return null;
  if (e.ev === 'error') return true;
  if (e.ev === 'notice') return false;
  return null;
}

/** The composer's field holds nothing he typed since (his words come back only into an empty field). */
function fieldEmpty(): boolean {
  return (document.querySelector('[data-testid="composer.field"]')?.textContent ?? '').trim() === '';
}

/** A card of a project, as the composer answers it. */
export interface CardAt {
  readonly project: string;
  readonly card: number;
  /** who asked it (inbox picks (3): his row in this thread when the asker's is elsewhere) */
  readonly asker?: string;
  /** the hub closed it while his words were typed: they go to `asker` as a plain message */
  readonly closed?: boolean;
  /** an item only its options decide (inbox/model.ts freeWords): his words refuse a tool call's
   *  approval ('deny'), else steer `asker` ('steer'); either way the item closes */
  readonly free?: 'deny' | 'steer';
  /** the pink border's tip ('your words answer search's question · esc writes to bise') */
  readonly tip?: string;
}

/**
 * The strip's 'to perf' in the accent (his m_44704, designer m_44890): his words go to an agent
 * that is not main with the border alone (pinkState's quiet): at rest with no @name lead, or as an
 * open item's free text ('to perf · the approval waits for 1 or 2', a closed question); never
 * main's, never while a question's own full pink says it. Pure.
 */
export function toAccent(ps: { quiet: boolean }, atTo: string | null): boolean {
  return ps.quiet && atTo === null;
}

/** One thing his free text does to an open item: a message to an agent, or a gesture on the card. */
export type FreeAct = { readonly act: 'send'; readonly send: SendIntent } | { readonly act: 'do'; readonly do: CardIntent };
type SendIntent = Parameters<ReturnType<typeof useStore>['send']>[0];
type CardIntent = Parameters<ReturnType<typeof useStore>['do']>[0];

/**
 * His words sent with item `a` open (his rule m_44704, the TUI's cards.rs submit): every item takes
 * free text and closes. A question: his answer (the hub's one /answer path, R41; `here` his thread,
 * so his row shows there too: inbox picks (3)). A tool call's approval: refused, his text the reason
 * its agent reads (the hub's answer `deny: <why>`, which closes it). Any other item only its options
 * decide: his text steers its asker ('now', as typed in its thread) and the item closes. One the hub
 * closed while he typed: his words to its asker alone (card-draft-keep). Pure: the composer runs
 * these through the store's one send and card paths.
 */
export function cardActs(a: CardAt, out: Outgoing, here: { project: string; agent: string }): readonly FreeAct[] {
  const parts = out.parts === undefined ? {} : { parts: out.parts };
  const steer = (to: string): FreeAct => ({ act: 'send', send: { project: a.project, agent: to, text: out.text, mode: 'now', ...parts } });
  if (a.closed === true && a.asker !== undefined) return [steer(a.asker)];
  if (a.free === 'deny') return [{ act: 'do', do: { i: 'answer', project: a.project, card: a.card, reply: `deny: ${out.text}`, ...parts } }];
  if (a.free === 'steer' && a.asker !== undefined) return [steer(a.asker), { act: 'do', do: { i: 'close', project: a.project, card: a.card } }];
  const from = a.asker === undefined ? {} : { from: { at: here } };
  return [{ act: 'do', do: { i: 'answer', project: a.project, card: a.card, reply: out.text, ...parts, ...from } }];
}

/** The inbox block's 'where am i' on bise's own view (no project, no agent: 'esc to bise instead'). */
export const HERE_BISE = { project: null, agent: null } as const;

/** The card his words answer while the block has one open (an approval: the card his words go
 *  to the asker of, as a message), else none. Pure. */
export function answerOf(a: { project: string; card?: number; answer: number | null; asker?: string; closed?: boolean; free?: 'deny' | 'steer'; tip?: string } | null): CardAt | null {
  if (a === null) return null;
  // an approval or a choice open: his free text refuses or steers, and closes it (his rule m_44704)
  if (a.free !== undefined && a.card !== undefined && a.asker !== undefined) return { project: a.project, card: a.card, asker: a.asker, free: a.free };
  // closed by the hub while he typed: its draft key stays, his words go to the asker (card-draft-keep)
  if (a.closed === true && a.card !== undefined) return { project: a.project, card: a.card, closed: true, ...(a.asker === undefined ? {} : { asker: a.asker }) };
  if (a.answer === null) return null;
  return { project: a.project, card: a.answer, ...(a.asker === undefined ? {} : { asker: a.asker }), ...(a.tip === undefined ? {} : { tip: a.tip }) };
}
