// Voice mode as the thread mounts it (designer's voice-window page (1), ambient's spec m_11632):
// useVoiceHere says whether this thread's composer gives its box to the bar ('pane'), comes back
// while he types ('typing': the screen gives it voice's label, keys and a grey pearl, VoiceTyping
// takes tab and esc), or stays ('off'). ThreadVoice draws the bar fed by the store (useVoiceMode,
// the levels, the voice role's model) and runs its acts (store.voiceMute / voiceType / voiceSend /
// voiceCut / voiceMode); it notes each turn he said (said.ts) for his bubble's 'said · 0:06'.
// useVoiceLit gives the thread's agent answer its spoken part (amb-core's voice_mode.lit).
// While it is on here, fn's talk is refused by the core (its error on talk_start): the label line
// says 'fn talks to bise again when you leave voice mode' for 3 s. Owner: amb-home (the page's v2: amb-web).
import { useEffect, useRef, useState } from 'react';
import { LivePearl } from '@/pearl';
import { useLevel, useRoles, useStore, useVoiceMode } from '@/store';
import { VoicePane } from './VoicePane';
import { FN_WAITS, FN_WAITS_MS, modelOf, splitLit, stateOf, type Act, type VoiceState } from './model';
import { noteSaid } from './said';

export type VoiceHere = 'off' | 'pane' | 'typing';

/** How long after a turn sent with no words his next bubble still takes its 'said' time. */
const UNWORDED_MS = 30_000;

/** Voice mode on this agent's thread: the bar, the composer back while he types, or off. */
export function useVoiceHere(project: string, agent: string): VoiceHere {
  const v = useVoiceMode();
  if (v === null || !v.on || v.project !== project || v.agent !== agent) return 'off';
  return v.state === 'typing' ? 'typing' : 'pane';
}

/** The window's runner of the bar's acts on this agent. */
function useActs(project: string, agent: string): (a: Act) => void {
  const store = useStore();
  return (a: Act) => {
    switch (a) {
      case 'send':
        store.voiceSend();
        return;
      case 'cut':
        store.voiceCut();
        return;
      case 'mute':
        store.voiceMute(true);
        return;
      case 'unmute':
        store.voiceMute(false);
        return;
      case 'type':
        store.voiceType(true);
        return;
      case 'voice':
        store.voiceType(false);
        return;
      case 'leave':
        store.voiceMode(project, agent, false);
        return;
      case 'retry':
        store.voiceMode(project, agent, true);
        return;
      case 'settings':
        window.bise.privacySettings('microphone');
        return;
      case 'think':
        // a hold while it is his turn keeps the floor: the core waits (no send); nothing to tell it
        return;
    }
  };
}

/** The bar in the composer's place. */
export function ThreadVoice({ project, agent }: { project: string; agent: string }) {
  const store = useStore();
  const v = useVoiceMode();
  const you = useLevel('you');
  const them = useLevel('agent');
  const roles = useRoles();
  const act = useActs(project, agent);
  const [waits, setWaits] = useState(false);
  const here = v !== null && v.on && v.project === project && v.agent === agent;
  const state: VoiceState = here ? stateOf(v.state) : 'listening';
  const heard = here ? (v.heard ?? null) : null;

  // fn pressed while it is on: the core refuses the talk; the label says so for 3 s
  useEffect(() => {
    if (!here) return;
    let t: ReturnType<typeof setTimeout> | null = null;
    const off = store.onEvent((ev) => {
      if (ev.ev !== 'error' || !('cmd' in ev) || ev.cmd !== 'talk_start') return;
      setWaits(true);
      if (t !== null) clearTimeout(t);
      t = setTimeout(() => {
        setWaits(false);
      }, FN_WAITS_MS);
    });
    return () => {
      off();
      if (t !== null) clearTimeout(t);
    };
  }, [store, here]);

  // his turn: from the first words heard to the send, its words and how long he talked (said.ts)
  const turn = useRef<{ at: number; words: string } | null>(null);
  // muted while it spoke: its voice goes on ('speaking · ○ muted')
  const [spoke, setSpoke] = useState(false);
  const [last, setLast] = useState<VoiceState | null>(null);
  if (state !== last) {
    setLast(state);
    if (state !== 'muted') setSpoke(state === 'speaking');
  }
  // a turn sent with no words in the core's events (its listener can give them only at the end):
  // how long he talked, for his next bubble in this thread (the hub's 'you' entry carries the words)
  const unworded = useRef<{ ms: number; at: number } | null>(null);
  // read from every voice_mode event, not from the rendered state: the core's hearing -> sending can
  // land in one batch of events, and a render would only see 'sending' (no turn to note)
  useEffect(
    () =>
      store.onEvent((ev) => {
        if (ev.ev === 'entry' && ev.project === project && ev.agent === agent && ev.entry.kind === 'you') {
          const u = unworded.current;
          unworded.current = null;
          if (u !== null && Date.now() - u.at < UNWORDED_MS) noteSaid(project, agent, ev.entry.text, u.ms);
          return;
        }
        if (ev.ev !== 'voice_mode' || ev.project !== project || ev.agent !== agent) return;
        const s = ev.on ? stateOf(ev.state) : 'listening';
        const words = ev.heard ?? null;
        if (s === 'hearing') {
          turn.current = { at: turn.current?.at ?? Date.now(), words: words ?? turn.current?.words ?? '' };
          return;
        }
        if (s === 'sending' && words !== null && turn.current !== null) turn.current = { ...turn.current, words };
        if ((s === 'sending' || s === 'thinking') && turn.current !== null) {
          const ms = Date.now() - turn.current.at;
          if (turn.current.words.trim() === '') unworded.current = { ms, at: Date.now() };
          else noteSaid(project, agent, turn.current.words, ms);
          turn.current = null;
        }
        if (s === 'listening' || s === 'failed') turn.current = null;
      }),
    [store, project, agent],
  );

  if (!here) return null;
  if (state === 'typing') return <VoiceTyping onAct={act} />;
  const model = modelOf(roles.find((r) => r.role === 'voice')?.model);
  return (
    <VoicePane
      state={state}
      agent={agent}
      heard={heard}
      fail={v.fail ?? null}
      model={model}
      speaking={state === 'muted' && spoke}
      you={you}
      them={them}
      label={waits ? FN_WAITS : null}
      onAct={act}
    />
  );
}

/** A list over the composer (its @ $ / list, a picker) keeps its own tab and esc. */
const listOpen = (): boolean => document.querySelector('.b-composer-pick, [role="listbox"][data-open="true"]') !== null;

/** While he types: tab goes back to voice, esc leaves (the page's key line); a list open keeps them. */
function VoiceTyping({ onAct }: { onAct: (a: Act) => void }) {
  const acts = useRef(onAct);
  useEffect(() => {
    acts.current = onAct;
  });
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey || e.isComposing || listOpen()) return;
      if (e.key === 'Tab' && !e.shiftKey) {
        e.preventDefault();
        e.stopPropagation();
        acts.current('voice');
      } else if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        acts.current('leave');
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('keydown', onKey, true);
    };
  }, []);
  return null;
}

/** The composer's grey small pearl while he types in voice mode (the page's typing tab). */
export function VoiceTypePearl() {
  return (
    <span className="vm-type-pearl" data-testid="voice.typing.pearl" aria-hidden>
      <LivePearl mood="still" size={20} />
    </span>
  );
}

/** An agent answer as voice mode lights it: said in ink, the rest faint, and the line under it
 *  ('∿ speaking' while it talks, 'you cut in' when he did); null: drawn as any answer (not this
 *  entry, nothing lit, or all of it said with no cut). `text` is the row's shown text, a suffix of
 *  the entry's `full` text the core counted in. */
export interface VoiceLit {
  readonly said: string;
  readonly unsaid: string;
  readonly mark: 'speaking' | 'cut' | null;
}

export function useVoiceLit(project: string, agent: string, pos: number, full: string, text: string): VoiceLit | null {
  const v = useVoiceMode();
  if (v === null || !v.on || v.project !== project || v.agent !== agent) return null;
  // amb-core's voice_mode.lit (c2ac78ba, 26647efd): the entry at pos, its first `said` UTF-16 units said
  const lit = v.lit;
  if (lit == null || lit.pos !== pos) return null;
  const cut = lit.cut === true;
  const state = stateOf(v.state);
  const talking = state === 'speaking' || (state === 'muted' && v.said != null);
  const drop = full.length - text.length;
  const [said, unsaid] = splitLit(text, lit.said - Math.max(0, drop));
  if (unsaid === '' && !cut && !talking) return null;
  return { said, unsaid, mark: cut ? 'cut' : talking ? 'speaking' : null };
}
