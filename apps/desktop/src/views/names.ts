// src/views · a view's names in words (designer m_50385): a tool's name as he reads it
// ('show_board' -> 'show board'), the permissions in a sentence ('the camera and the
// microphone'), and the card's links keep the focus where it is. The card, the tab and the plugins page share them. Owner: mcp-apps-host.
import { views } from '@/proto/words_views.gen';
import { permRow } from '@/shared/viewperms';

/** A tool's or a view's name humanised: '_' and '-' are spaces. Pure. */
export function humanName(name: string): string {
  return name.replace(/[_-]+/g, ' ').trim();
}

/** A view's name from its ui:// uri: its last part, humanised ('ui://fake-app/counter' -> 'counter'). Pure. */
export function uriName(uri: string): string {
  return humanName(uri.split('/').filter((x) => x !== '').at(-1) ?? uri);
}

/** One permission in a sentence ('the microphone', 'MIDI'; the generated table's); an unknown name as it is. Pure. */
export function permIn(p: string): string {
  return permRow(p)?.inSentence ?? p;
}

/** Permissions in a sentence: 'the camera and the microphone', 'the camera, MIDI and the clipboard'. Pure. */
export function permsIn(perms: readonly string[]): string {
  const ns = perms.map(permIn);
  const last = ns.at(-1) ?? '';
  return ns.length <= 1 ? last : views.list_and({ head: ns.slice(0, -1).join(', '), last });
}

/**
 * A card's link presses quietly: no focus (Tab still reaches it) and its press never reaches the
 * thread list's own (which holds the pressed row's y, a row toggle's rule): with both, the list
 * scrolled 169 px between his press and its release, the click landed on another row and the
 * panel never opened (ui-review m_50430).
 */
export const quiet = {
  onMouseDown: (e: { preventDefault: () => void }): void => {
    e.preventDefault();
  },
  onPointerDown: (e: { stopPropagation: () => void }): void => {
    e.stopPropagation();
  },
} as const;

/**
 * 'open in the panel' and 'show here' press without focus too, but their press reaches the thread
 * list: the card is its row's `data-hold` part, so the list holds the card's top at its y while it
 * shrinks into its line or grows back, and keeps a list stuck to its end there (ThreadList's
 * pressHold; designer m_50494, mcp-apps-hold). Holding the card itself, nothing moves between his
 * press and its release.
 */
export const holds = { onMouseDown: quiet.onMouseDown } as const;
