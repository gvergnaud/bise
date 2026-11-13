// Voice mode and the composer's mic (amb-core's, store.ts's split, ambient-lead m_22445).
import {
  type Dictation,
} from "./types";
import { StoreReads } from "./reads";

/** A stopped dictation's final is the core's once its listener flushed, up to TRANSCRIBE_TIMEOUT
 *  (120 s) after the listener's last sign; past this with no word from the core, the window ends
 *  it. Each `dictation` of a stopped take (a long take's segment heard) starts the wait again, so
 *  it grows with the take's length (dictate-nocap, architect m_64474). Never shorter than the
 *  core's wait: a 5 s cut threw his words away (ambient-lead m_22121). */
export const DICTATE_STOP_WAIT_MS = 125_000;

export class StoreVoice extends StoreReads {
  /**
   * Voice mode with the agent in view (the TUI's ctrl+r twice): the core's
   * mic and speaker, his turns to that agent, its answers said aloud. Another
   * agent while on: it follows him there; off: it ends (useVoiceMode's last
   * state has on: false and the note).
   */
  voiceMode(project: string, agent: string, on: boolean): void {
    this.command({ cmd: "voice_mode", project, agent, on });
  }
  /** His mic off (m) or on again; the agent still speaks. */
  voiceMute(on: boolean): void {
    this.command({ cmd: "voice_mute", on });
  }
  /** He types (tab): the mic waits; his normal send to that agent gets its answer said. */
  voiceType(on: boolean): void {
    this.command({ cmd: "voice_type", on });
  }
  /** What he said goes now (the TUI's space tap). */
  voiceSend(): void {
    this.command({ cmd: "voice_send" });
  }
  /** Cut the agent off: its voice stops, its running turn is stopped. */
  voiceCut(): void {
    this.command({ cmd: "voice_cut" });
  }

  /**
   * The composer's mic (bar I): the core records and sends this window the
   * words (useDictation(id)); never to main. Returns the dictation's id.
   */
  dictateStart(): string {
    this.cid += 1;
    const id = `d${String(this.cid)}-${String(this.now())}`;
    this.dictating.add(id);
    this.put(`dictation:${id}`, {
      text: "",
      final: false,
      cancelled: false,
      error: null,
    } satisfies Dictation);
    this.command({ cmd: "dictate_start", id });
    return id;
  }
  /**
   * The mic off; the final words come in one last `dictation`. A core that never answers (its mic
   * stuck opening, the start lost) never leaves 'listening…' up: after DICTATE_STOP_WAIT_MS the
   * dictation ends here with the words it has and a note (amb-tools' run on the cut, m_20100).
   */
  dictateStop(id: string): void {
    if (!this.dictating.has(id)) return;
    this.command({ cmd: "dictate_stop", id });
    this.dictateWait(id);
  }
  /** A take the core kept (its final with no words or an error, `kept`): transcribed once more,
   *  one more final for that id (amb-core 276074b2); the same wait as a stop. A take this window
   *  never saw (a draft's, after a relaunch or a reload: dictation-keep) is asked for too: the core
   *  transcribes its sound again, or sends its final when it still transcribes it. */
  dictateRetry(id: string): void {
    const d = this.dictation(id);
    if (this.dictating.has(id) || (d !== null && d.kept !== true)) return;
    this.dictating.add(id);
    this.put(`dictation:${id}`, {
      text: "",
      final: false,
      cancelled: false,
      error: null,
    } satisfies Dictation);
    this.command({ cmd: "dictate_retry", id });
    this.dictateWait(id);
  }
  /** A kept take he let go (esc, his send, its line gone): the core drops it. */
  dictateDrop(id: string): void {
    if (this.dictating.has(id) || this.dictation(id)?.kept !== true) return;
    this.command({ cmd: "dictate_cancel", id });
    this.put(`dictation:${id}`, {
      text: "",
      final: true,
      cancelled: true,
      error: null,
    } satisfies Dictation);
  }
  /** The take's words are in his draft: the core lets its sound go (it keeps it on disk until
   *  then: dictation-keep). Nothing changes here. */
  dictateDone(id: string): void {
    if (this.dictating.has(id)) return;
    this.command({ cmd: "dictate_cancel", id });
  }
  private dictateWait(id: string): void {
    clearTimeout(this.dictateWaits.get(id));
    this.dictateWaits.set(
      id,
      setTimeout(() => {
        this.dictateWaits.delete(id);
        if (!this.dictating.has(id)) return;
        this.dictating.delete(id);
        this.put(`dictation:${id}`, {
          text: this.dictation(id)?.text ?? "",
          final: true,
          cancelled: false,
          error: "the mic didn't answer.",
        } satisfies Dictation);
      }, DICTATE_STOP_WAIT_MS),
    );
  }
  /** @internal for apply.ts: the core said something of a stopped take (a segment heard, its
   *  final): its wait starts again, or ends with the final. A take still recording has none. */
  dictateHeard(id: string, final: boolean): void {
    if (!this.dictateWaits.has(id)) return;
    if (final) {
      clearTimeout(this.dictateWaits.get(id));
      this.dictateWaits.delete(id);
    } else this.dictateWait(id);
  }
  /** esc: the dictation is over at once, here; the core is told, its late answer is ignored
   *  (a core that never answers leaves nothing up). */
  dictateCancel(id: string): void {
    if (!this.dictating.has(id)) return;
    this.command({ cmd: "dictate_cancel", id });
    this.dictating.delete(id);
    this.put(`dictation:${id}`, {
      text: "",
      final: true,
      cancelled: true,
      error: null,
    } satisfies Dictation);
  }
}
