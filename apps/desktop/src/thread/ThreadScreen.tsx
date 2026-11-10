// One agent's thread: its entries in the reading column, its current step, ⌘F, and the composer.
// Its head is the window's bar since bise fait mac (QA §4): nm-frame's title and subtitle, this
// screen's '…' (the agent's menu) through <InBar>, its rename inline in the title through <InTitle>;
// the column sits where the window's sideLayout puts it (column.ts). Owner: nm-thread.
// Bar ⛔3 (a send shows at once),
// ⛔4 (readable answers), ⛔6 (ctrl+c stops the turn), 3.3 (writing to one
// agent: the composer's target is this agent), 3.4 (⌘F).
//
// What re-renders what (the reset's flicker rule): the list re-renders
// when its keys change (an entry or a send comes or goes); each row reads
// its own data; the composer is memo'd with stable props, so neither an
// entry, a step nor a send ever re-renders it.
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { Button, Marker } from '@/ds';
import type { ComposerActions } from '@/composer';
import type { AppCmd, Card } from '@/proto';
import { threadKey, useAgents, useCards, useCoreState, usePrefs, useProjects, useStore, useThreadOrder, useTyping } from '@/store';
import { prefsOf } from '@/settings';
import { HomeNote, useHomeNote } from '@/setup/TelemetryNote';
import { usePartialText } from '@/shared/partial';
import { useMenuAction } from '@/app/menu';
import { useRoute } from '@/app/router';
import { InBar } from '@/app/barslot';
import { BarMore } from '@/app/barbutton';
import { useEntityMenu } from '@/app/menus/use';
import { MAIN } from '@/app/route';
import { useShowsMainAtEnd } from '@/app/seen';
import { RefusedLine } from '@/app/Refused';
import { keep } from '@/app/keep';
import { Tip } from '@/setup';
import { useTour } from '@/setup/useTour';
import { onQuotes, takeQuotes } from '@/review/quote';
import { QuotePop } from '@/quote/QuotePop';
import { Hint, hintUsed, hintsUserMessage, useHint } from '@/app/hints';
import { HubNoteRow, InboxBlock, type Answering } from '@/inbox';
import { shownName, turnRuns } from '@/agents/model';
import { StartingLine } from './StartingLine';
import { useColumn } from './column';
import { TitleRename } from './Rename';
import { PauseLine } from './PauseLine';
import { LabelMark } from './LabelMark';
import { StepLine } from './live-step';
import { composerWords, emptyEnter, escTail, nothingOwnYet, usageLine, workingRowShows } from './head';
import { useTabPlace, useTabWords } from './ComposerTab';
import { SignInLine, useThreadSignIn } from './signin';
import { type ThreadListHandle } from './ThreadList';
import { ComputerLine } from './ComputerLine';
import { entryKey } from './entry';
import { thread } from '@/proto/words.gen';
import { flashEntry, useAnswerReveal } from './useAnswerReveal';
import { useFindGo } from './useFindGo';
import { quoteSourcePos } from './quote';
import { QUOTE_SOURCE_EVENT } from '@/composer/pm/AttachList';
import { FindBar } from './FindBar';
import { createFindMark, FindMarkContext } from './findmark';
import { trackRoutes, useHeld } from './routes';
import { OpenArtifactContext } from './openArtifact';
import { ThreadDrop } from '@/artifacts/Drops';
import { createFolds, FoldsContext } from './folds';
import { ThreadVoice, TYPING_END, typingLabelOf, useVoiceHere, VoiceTypePearl } from '@/voicemode';
import { Rows } from './ThreadRows';
import { HomeSlot } from '@/composer/home/Home';
import { useThreadQuote } from './threadQuote';
import { answerOf, HERE_BISE, ThreadComposer } from './ThreadComposer';

export interface ThreadScreenProps {
  project: string;
  agent: string;
  /** bise's own main: 'bise' with its pearl */
  bise?: boolean;
  /** the project's name for the head and the composer's 'you → …' */
  projectName?: string;
  /**
   * The artifact open beside this thread (designer's artifact-open page): its row is marked 'open',
   * the one composer is about it ('to perf · about “…”'), main's inbox is not on this screen.
   */
  artifact?: {
    id: string;
    title: string;
    url: string;
    to: string;
    placeholder: string;
    /**
     * whom the one composer writes to: the maker (this thread's agent), or the project's main when
     * the maker is archived (designer m_20453: its own thread stays in the centre, dim, '↺ restore'
     * in its head; only the composer goes to main)
     */
    agent?: string;
    /** its words go to a live sub-agent (art-target): the composer's pink with this tip, none: plain */
    pink?: string;
    /** the tab's right while pink: 'esc to bise instead' */
    end?: string;
  };
}

export function ThreadScreen({ project, agent, bise = false, projectName, artifact }: ThreadScreenProps) {
  const store = useStore();
  const tkey = threadKey(project, agent);

  // the thread's pages and live entries while it is shown, asked again
  // when a restarted core comes up (a new core holds no subscription)
  const core = useCoreState();
  const live = core !== 'down';
  useEffect(() => {
    if (!live) return;
    store.subscribeThread(project, agent);
    return () => {
      store.unsubscribeThread(project, agent);
    };
  }, [store, project, agent, live]);

  // bise's routing holds of his sends here (read before the first send)
  trackRoutes(store);
  const held = useHeld(store, tkey);

  const agents = useAgents(project);
  const me = agents.find((a) => a.name === agent);
  // beside an open artifact the composer writes to its maker: its row for the tab and the model menu
  const addressee = artifact?.agent == null ? me : agents.find((a) => a.name === artifact.agent);
  // voice mode on this agent: its pane takes the composer's place (amb-home's src/voicemode)
  const voice = useVoiceHere(project, agent);
  const typing = useTyping(tkey);
  const busy = turnRuns(me?.status) || typing !== '';
  // an open artifact's block lists only its maker's own items (this thread's agent)
  const onlyMaker = useCallback((c: Card) => c.project === project && c.agent === agent, [project, agent]);

  // ⌘F's bar stays open across a reload (keep.ts: an open panel, no age); its folds too
  const findKey = `find.open:${project}/${agent}`;
  const [findOpen, setFindOpenState] = useState(() => keep.view(findKey) === true);
  const setFindOpen = useCallback(
    (on: boolean) => {
      setFindOpenState(on);
      keep.setView(findKey, on ? true : undefined);
    },
    [findKey],
  );
  const [mark] = useState(createFindMark);
  const [folds] = useState(() => createFolds(`${project}/${agent}`));
  const list = useRef<ThreadListHandle>(null);
  // a quote's row in the composer's attachment list: its words' entry here comes into view and flashes
  useEffect(() => {
    const go = (e: Event): void => {
      const { text } = (e as CustomEvent<{ from: string; text: string }>).detail;
      const pos = quoteSourcePos(store, tkey, text);
      if (pos === null) return;
      list.current?.scrollTo(entryKey(pos));
      window.setTimeout(() => {
        flashEntry(pos);
      }, 120);
    };
    window.addEventListener(QUOTE_SOURCE_EVENT, go);
    return () => {
      window.removeEventListener(QUOTE_SOURCE_EVENT, go);
    };
  }, [store, tkey]);
  // answered-link: a '✓ answered' row's click brings his answer here into view (answerlink.ts)
  const answerMiss = useAnswerReveal(store, project, agent, list);
  // ⌘F: the current match into view, its page read around it when it isn't held (useFindGo.ts)
  const findGo = useFindGo(store, project, agent, list);
  // H10 (designer m_19835): scrolled up to read, the inbox block is one line and the composer's keys line hides
  const [reading, setReading] = useState(false);
  // his bug 3: a selection in the thread, then a key, quotes it in the composer
  const section = useRef<HTMLElement>(null);
  const composer = useRef<ComposerActions | null>(null);
  // V7: lines quoted in the review come in as chips, then the key he typed
  const takeHanded = useCallback(
    (a: ComposerActions) => {
      const q = takeQuotes(project, agent);
      if (q === null) return;
      // after the composer's own listeners are up (its strip reads the chips on an update)
      setTimeout(() => {
        for (const x of q.quotes) a.quote(x.text, q.from, x.at);
        a.insertText(q.key);
        a.focusEnd();
      }, 0);
    },
    [project, agent],
  );
  const onComposer = useCallback(
    (a: ComposerActions) => {
      composer.current = a;
      takeHanded(a);
    },
    [takeHanded],
  );
  // the review in the panel beside this thread (page pr-guided): its quotes come in here at once
  useEffect(
    () =>
      onQuotes(() => {
        if (composer.current !== null) takeHanded(composer.current);
      }),
    [takeHanded],
  );
  // the column where the window's sideLayout puts it (column.ts); the selection pop stays in it
  const column = useColumn(section);
  const quote = useThreadQuote(section, composer, agent, column.place);
  const quoting = quote.quoting;
  // L25: the demo's tips; 'inside' and 'end' sit at the thread's top, his send moves the tour
  const tour = useTour(project);
  const tourSent = useRef(tour.sent);
  useEffect(() => {
    tourSent.current = tour.sent;
  }, [tour.sent]);
  // Gabriel's note, once, for an install that never saw it (usage-metrics-plan step 6): docked over
  // bise's composer until 'Got it' or his first message
  const prefs = prefsOf(usePrefs());
  const [homeNote, dropHomeNote] = useHomeNote(prefs);
  const sendCmd = useCallback((cmd: AppCmd) => { store.command(cmd); }, [store]);
  const onSent = useCallback(() => {
    tourSent.current();
    // S36: his message puts the one-time hints away
    hintsUserMessage();
    dropHomeNote();
  }, [dropHomeNote]);
  const topTip = (tour.tip === 'inside' && tour.snap.focus === agent) || tour.tip === 'end' ? tour.tip : null;
  useMenuAction('find', () => {
    setFindOpen(true);
  });
  // the window's keys (app/keys.ts): ⌘E everything folded, ⌘. stop,
  // end back to the bottom, ctrl+y copy a code block
  useMenuAction('fold-all', () => {
    // the row at the top of the view keeps its y while everything opens or closes (designer m_17092)
    list.current?.holdView();
    folds.toggle();
  });
  useMenuAction('to-end', () => {
    list.current?.scrollToEnd();
  });
  useMenuAction('copy-code', () => {
    const code = codeToCopy(document);
    if (code !== undefined) void (window as Partial<Window>).bise?.copy(code);
  });

  // ⛔6: a stop he asked, shown until the turn's end or a new entry
  const [stopping, setStopping] = useState(false);
  const stop = useCallback(() => {
    store.stop(project, agent);
    setStopping(true);
  }, [store, project, agent]);
  if (stopping && !busy) setStopping(false);
  useMenuAction('stop', () => {
    if (busy) stop();
  });

  /**
   * The thread's esc, once no list or popup took it (R18, the TUI's order): while bise holds his
   * words, keep them with bise (route_cancel). It never stops a turn: ctrl+c does, as the TUI's
   * (his ask main m_22205; rust/tui/src/sb/keys.rs:104). The composer calls it itself (ProseMirror
   * preventDefaults every Escape); an esc from elsewhere bubbles here.
   */
  // memo'd: the composer is (a new entry never re-renders it)
  const escape = useCallback((): boolean => {
    if (findOpen) return false;
    if (held === undefined) return false;
    store.command({ cmd: 'route_cancel', project: held.home, rid: held.rid });
    return true;
  }, [findOpen, held, store]);
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape' && !e.defaultPrevented) {
      if (escape()) e.preventDefault();
    } else if (e.key === 'c' && e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey && !e.defaultPrevented) {
      // the TUI's ctrl+c (rust/tui/src/sb/keys.rs:104, his ask main m_22205): the one key that stops
      // the turn; copy is ⌘C on the Mac, so ctrl+c is free, but with text selected it does nothing
      // (he may be copying by habit)
      if (busy && (window.getSelection()?.isCollapsed ?? true)) {
        e.preventDefault();
        stop();
      }
    } else if (e.key === 'f' && e.metaKey && !e.shiftKey && !e.altKey && !e.ctrlKey) {
      e.preventDefault();
      setFindOpen(true);
    }
  };

  // a project's main is bise (ambient m_11437): the composer's words name it so
  const title = bise || agent !== 'main' ? shownName(bise ? 'main' : agent) : 'bise';
  // b1/b2 (his card #476): the inbox block over the composer tells what his words do while one of
  // its items is open in place: a question's answer ('answering perf · …'), or a plain message
  // with what an approval waits for; esc folds the item and his words are a message again
  const [answering, setAnswering] = useState<Answering | null>(null);
  // R15: an expired sign-in stopped this agent and his browser is on it: c on the empty composer
  // copies the link, esc cancels (the TUI's resign.rs keys)
  const cardsHere = useCards(project);
  const signIn = useThreadSignIn(emptyEnter(cardsHere, agent) !== null);
  const onKeyDownCapture = (e: KeyboardEvent) => {
    const field = e.target instanceof HTMLElement ? e.target.closest('[data-testid="composer.field"]') : null;
    if (signIn.on && field !== null && field.textContent.trim() === '' && !e.metaKey && !e.ctrlKey && !e.altKey) {
      if (e.key === 'Escape' || e.key === 'c') {
        e.preventDefault();
        e.stopPropagation();
        if (e.key === 'c') signIn.copy();
        else signIn.cancel();
        return;
      }
    }
  };
  // designer's v6 (5): an agent whose thread has nothing of its own yet (his first message at most) is starting
  const order = useThreadOrder(tkey);
  // once it writes (a streamed reply, live.ts) it isn't starting any more, even between the
  // partial's clear and its entry (sticky for this thread: the line never comes back mid-turn)
  const writing = usePartialText(project, agent) !== null;
  const [wrote, setWrote] = useState<string | null>(null);
  if (writing && wrote !== tkey) setWrote(tkey);
  const fresh = !bise && agent !== MAIN && me !== undefined && !me.archived && order.loaded && nothingOwnYet(order.order.map((pos) => store.entry(tkey, pos)?.kind)) && me.status === 'working' && me.waits === 0 && !writing && wrote !== tkey;
  // the gust beside 'to bise' (designer's desktop-working v2): the composer's addressee's, kept
  // stable so the memoised composer doesn't draw again for it (the mark reads its agent itself)
  const markAgent = artifact?.agent ?? agent;
  const markName = markAgent === agent ? title : markAgent;
  const startingMark = fresh && markAgent === agent;
  const labelMark = useMemo(
    () => <LabelMark project={project} agent={markAgent} name={markName} starting={startingMark} />,
    [project, markAgent, markName, startingMark],
  );
  // the composer's tab (designer's composer-strip A): the addressee's place and context; the crumb
  // names the project on a thread, so the tab never does; 'idle' and the model are gone from it
  // (the model is the box's menu, working is the gust)
  const addr = markAgent === agent ? me : addressee;
  const tabPlace = useTabPlace(project, addr, bise);
  const tabContext = usageLine(addr?.usage?.words);
  const tabLabel = voice === 'typing' ? typingLabelOf(agent) : answering !== null ? answering.label : artifact !== undefined ? artifact.to : composerWords(title).label;
  // one object while its words hold, so a new entry never draws the memoised composer again
  const tab = useTabWords(tabLabel, tabPlace, tabContext, markName);
  // the head is the window's bar (bise fait mac): its title and subtitle are nm-frame's; this
  // thread puts the agent's '…' there (and a project main's counts, tui-parity W2), and takes the
  // menu's Rename in the bar's title, when it is the route's screen (not a panel's tab)
  const route = useRoute();
  const inView = route.screen === 'thread' && route.project === project && route.agent === agent;
  // a project main's thread in view at its newest row: what it wrote for him is seen (app/seen.ts)
  useShowsMainAtEnd(project, inView && agent === MAIN && !reading);
  const menu = useEntityMenu({ kind: 'agent', project, agent });
  const ours = !bise && me !== undefined && !me.archived;
  return (
    <FoldsContext value={folds}>
    <FindMarkContext value={mark}>
    <OpenArtifactContext value={artifact?.id}>
      <section ref={section} className="th-screen" data-testid="thread.screen" data-project={project} data-agent={agent} data-quoting={quoting} data-answering={answering !== null} data-reading={reading} data-about={artifact !== undefined || undefined} data-dim={(artifact !== undefined && me?.archived === true) || undefined} onKeyDown={onKeyDown} onKeyDownCapture={onKeyDownCapture} style={column.style}>
        {inView && ours && (
          <InBar>
            <BarMore menu={menu} testId="thread.more" />
          </InBar>
        )}
        <TitleRename project={project} agent={agent} agents={agents} take={inView && ours && !me.main} />
        {/* an archived maker beside its artifact: its thread dim, a way to restore it on top (designer m_20453) */}
        {artifact !== undefined && me?.archived === true && agent !== MAIN && <History project={project} agent={agent} />}
        {/* S29/L19 W1: it drives his computer: one line under the head, from computer_live only */}
        {!bise && <ComputerLine project={project} agent={agent} />}
        {answerMiss && (
          <div className="th-answer-miss" data-testid="thread.answer-miss">
            {thread.answer_further_up}
          </div>
        )}
        {topTip !== null && (
          <div className="th-tip">
            <Tip tip={topTip} snap={tour.snap} />
          </div>
        )}
        {/* the list and, over an empty one, the first-run block centred in the column (his try:
            it sat bottom-left at the column's edge) */}
        <div className="th-main">
          <Rows tkey={tkey} project={project} agent={agent} bise={bise} listRef={list} onReading={setReading} />
          {/* main's empty thread: the empty home (composer/home, bise fait mac), on a project's welcome */}
          {agent === MAIN && me?.archived !== true && <HomeSlot tkey={tkey} project={project} composer={composer} bise={bise} />}
          {/* ⌘F: a compact bar over the list's top right (his m_25471) */}
          {findOpen && (
            <FindBar
              tkey={tkey}
              project={project}
              agent={agent}
              mark={mark}
              onGo={findGo}
              onClose={() => {
                setFindOpen(false);
              }}
            />
          )}
        </div>
        {/* in the thread's column like the lines under it (amb-web m_17316: it sat flush left) */}
        {stopping && (
          <div className="th-stopping" data-testid="thread.stopping">
            <Marker>stopping {agent}…</Marker>
          </div>
        )}
        {fresh && <StartingLine a={me} />}
        {/* a pause's 'continues when …' line (agent-waiting-states A); working, only the gust beside the label says it
            (designer's desktop-working v2, his m_21668: the working row went) */}
        {!fresh && <PauseLine a={me} project={project} />}
        {/* a typed command's result (the hub's plain line): one quiet row, never under the composer (amb-home) */}
        <HubNoteRow project={project} />
        {signIn.on && <SignInLine copied={signIn.copied} />}
        {/* the step at work, once: the growing fold '▸ <its words>  ∿ 10 s' (live-step.tsx) */}
        <StepLine tkey={tkey} project={project} agent={agent} working={workingRowShows(me)} typing={typing} />
        {/* W7/K24: the project's open items over the composer's divider (amb-home f44d082e) */}
        {/* its hub refused this core: the hub's words and a retry, over the composer (lead m_11347) */}
        <RefusedLine project={project} name={bise ? 'bise' : (projectName ?? project)} className="app-refused th-refused" testId="thread.refused" />
        <CardHint project={project} />
        {bise && artifact === undefined && <HomeNote prefs={prefs} shown={homeNote} dismiss={dropHomeNote} send={sendCmd} />}
        {/* b1/b2: every project's open items, one opened in place (amb-home's InboxBlock, eb3da983) */}
        {artifact === undefined ? (
          <InboxBlock here={bise ? HERE_BISE : { project, agent }} onAnswer={setAnswering} />
        ) : (
          // beside an open artifact: only the maker's own question, docked over its one composer
          // (designer's artifact-open (g)); main's inbox is not on this screen
          <InboxBlock here={{ project, agent: artifact.agent ?? agent }} only={onlyMaker} onAnswer={setAnswering} />
        )}
        {me?.archived === true && artifact === undefined ? (
          <History project={project} agent={agent} />
        ) : voice === 'pane' ? (
          <ThreadVoice project={project} agent={agent} />
        ) : (
          <>
          {voice === 'typing' && <ThreadVoice project={project} agent={agent} />}
          <ThreadComposer
            project={project}
            agent={artifact?.agent ?? agent}
            busy={busy}
            // an open item: the block's words (a question's 'answering perf · …', or an approval's
            // 'to bise · the merge waits for 1 or 2' where his words stay a plain message, ambient m_8773);
            // voice mode while he types: 'to perf · voice paused', 'tab back to voice' (designer's voice-window (1))
            tab={tab}
            tabEnd={voice === 'typing' ? TYPING_END : answering !== null ? answering.end : artifact?.end}
            me={addr}
            voiceTyping={voice === 'typing'}
            lead={voice === 'typing' ? <VoiceTypePearl /> : undefined}
            mark={labelMark}
            placeholder={answering?.placeholder ?? (artifact !== undefined ? artifact.placeholder : composerWords(title).placeholder)}
            // a sub-agent's: esc goes up to its project's main, bise (esc-up.ts); the line says so (his ask, designer m_41726)
            placeholderTail={escTail(agent, answering !== null || artifact !== undefined || voice === 'typing')}
            {...(artifact === undefined ? {} : { about: { id: artifact.id, title: artifact.title, url: artifact.url } })}
            aboutPink={voice === 'typing' ? undefined : artifact?.pink}
            answer={answerOf(answering)}
            onStop={stop}
            onEscape={escape}
            onReady={onComposer}
            onSent={onSent}
          />
          </>
        )}
        {quote.pop !== null && (
          <span className="q-float" style={{ left: quote.pop.x, top: quote.pop.y }}>
            <QuotePop testId="thread.quote.pop" onAsk={quote.ask} onCopy={quote.copy} />
          </span>
        )}
        {/* files dragged over the window: 'drop to give this file to perf', ONE zone over the rows
            AND the composer (his m_44892): the drop gives them to the composer's agent (designer's
            office-artifacts pick 4) and goes on to the window, where the composer makes them its
            chips once (paste.ts useDropAnywhere; over the zone the field never takes it); an
            archived thread takes none */}
        {(me?.archived !== true || artifact !== undefined) && (
          <ThreadDrop project={project} agent={artifact?.agent ?? agent} to={bise ? 'bise' : shownName(artifact?.agent ?? agent)} />
        )}
      </section>
    </OpenArtifactContext>
    </FindMarkContext>
    </FoldsContext>
  );
}

/**
 * ctrl+y's block (the TUI's codeblock.rs): the code block under the mouse,
 * else the newest one drawn in the thread. Its text, as written.
 */
export function codeToCopy(doc: Document): string | undefined {
  const hovered = doc.querySelector('.th-list pre:hover code, .th-list pre:hover');
  if (hovered !== null) return hovered.textContent;
  const all = doc.querySelectorAll('.th-list pre code');
  const last = all.item(all.length - 1) as Element | null;
  return last === null ? undefined : last.textContent;
}

/** The list: entries by pos, then his sends not confirmed yet. */

/** An archived agent's thread is history only (tui-parity C47): no composer, a way to restore it. */
function History({ project, agent }: { project: string; agent: string }) {
  const store = useStore();
  return (
    <div className="th-history" data-testid="thread.archived">
      {/* designer's ⌘K screen (amb-win m_11920): '<name> is archived · ⌘⏎ restore' */}
      <span>{thread.is_archived({ agent })}</span>
      <Button
        kind="ghost"
        onClick={() => {
          store.command({ cmd: 'unarchive', project, agent });
        }}
        testId="thread.restore"
      >
        <kbd>⌘⏎</kbd> {thread.restore}
      </Button>
    </div>
  );
}

/**
 * S36: the first card's one-time hint, over the thread's inbox strip; it goes when the
 * project has no open card any more (answered, closed) or after his next message.
 */
function CardHint({ project }: { project: string }) {
  const n = useProjects().find((p) => p.project === project)?.cards?.length ?? 0;
  const on = useHint('first_card', n > 0);
  useEffect(() => {
    if (n === 0) hintUsed('first_card');
  }, [n]);
  return on ? (
    <div className="th-hint">
      <Hint name="first_card" />
    </div>
  ) : null;
}
