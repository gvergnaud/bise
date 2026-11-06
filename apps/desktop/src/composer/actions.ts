// What the screen around a composer may ask of it (ComposerActions, handed over once mounted), and
// what its field tells the key bar (FieldBar). Types only.
import type { QuoteAt } from './message';
import type { Anchor } from '@/proto/anchor';

/** What the screen around a composer may ask of it (dictation, the send button, focus). */
export interface ComposerActions {
  submit(): void;
  /** Words at the caret (dictation's final text, a pick from elsewhere): editable, never sent. */
  insertText(text: string): void;
  /**
   * The field holds `text` alone, all of it selected, and has the focus (the first-run text's
   * "show me what you can do", BISE-284): typing replaces it, ⏎ sends it; never a send.
   */
  fill(text: string): void;
  /**
   * His words from elsewhere (an entry: a closed card's half answer, card-draft-keep), never lost:
   * into the field when it is empty, else on top of its history (↑ brings them back). Says where.
   */
  putBack(entry: string): 'field' | 'history';
  focus(): void;
  /** The focus with the caret at the draft's end (a press on the frame's padding or empty area, his round 2). */
  focusEnd(): void;
  /** All his words selected, with the focus: typing replaces them (the spotlight's kept draft on its next fn+space). */
  selectAll(): void;
  /** The field's content as an entry (entry.ts): his words and chips, to carry elsewhere. */
  entry(): string;
  /** The field emptied as by a send (its undo forgotten, the draft dropped): his words went elsewhere. */
  clear(): void;
  /** Files he picked by path (the '+' in the box's row): each a chip at the caret, an image as an image chip. */
  attach(paths: readonly string[]): void;
  /** The mic control: on, or off when on (bar I). */
  dictate(): void;
  /**
   * Text selected in the thread, as a quote chip at the caret (`❝ 1`, its strip says `from` and its lines), then the focus.
   * `at`: lines of a diff (V7, diffquote.rs): the strip says 'src/a.rs:12-14 · 3 lines' and the tag carries file / new / old.
   */
  quote(text: string, from: string, at?: QuoteAt): void;
  /**
   * A part of an artifact picked in the side panel (plan §9b) as a numbered attachment at the caret
   * ('❝ 2', its row 'Summary!C4:E4  q3-forecast.xlsx · v4'), then the focus; sent as Part::Anchor.
   */
  anchor(a: AnchorPick): void;
}

/** What the side panel's frame gave: the artifact, its version, where in it, the words, its file's name. */
export interface AnchorPick {
  readonly artifact: string;
  readonly version: number;
  readonly rev?: number;
  readonly anchor: Anchor;
  readonly quote?: string;
  readonly file: string;
}

/** What the field tells the key bar: a list open, text selected, images, text in it. */
export interface FieldBar {
  readonly fileList: boolean;
  readonly selected: boolean;
  readonly images: boolean;
  /** chips in the draft: the key line says backspace on one removes it (composer-attachments A) */
  readonly chips?: boolean;
  readonly typing: boolean;
  /** dictation: the mic on, then its last words coming */
  readonly dictating: 'recording' | 'transcribing' | null;
}
