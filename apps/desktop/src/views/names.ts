// src/views · a view's names in words (designer m_50385): a tool's name as he reads it
// ('show_board' -> 'show board'), the permissions in a sentence ('the camera and the
// microphone'). Pure; the card, the tab and the plugins page share them. Owner: mcp-apps-host.
import { views } from '@/proto/words_views.gen';

/** A tool's or a view's name humanised: '_' and '-' are spaces. Pure. */
export function humanName(name: string): string {
  return name.replace(/[_-]+/g, ' ').trim();
}

/** A view's name from its ui:// uri: its last part, humanised ('ui://fake-app/counter' -> 'counter'). Pure. */
export function uriName(uri: string): string {
  return humanName(uri.split('/').filter((x) => x !== '').at(-1) ?? uri);
}

/** One permission in a sentence ('the microphone', 'MIDI'). Pure. */
export function permIn(p: string): string {
  switch (p) {
    case 'camera':
      return views.missing_camera;
    case 'microphone':
      return views.missing_microphone;
    case 'midi':
      return views.missing_midi;
    case 'clipboardWrite':
      return views.missing_clipboard;
    default:
      return p;
  }
}

/** Permissions in a sentence: 'the camera and the microphone', 'the camera, MIDI and the clipboard'. Pure. */
export function permsIn(perms: readonly string[]): string {
  const ns = perms.map(permIn);
  const last = ns.at(-1) ?? '';
  return ns.length <= 1 ? last : views.list_and({ head: ns.slice(0, -1).join(', '), last });
}
