// Every message between an artifact's frame and the window, by its `bise` kind (architect m_30630):
// the desktop's copy of kit/office/frame.js's FRAME, one list both sides name. Here in shared/ so
// main (guard.ts's injected esc forwarder) and the renderer (artifacts/frameNote.ts, which exports
// it, and the law that checks it equals the kit's) read the same one. Owner: art-desktop.
//
// - anchor: a selection, frame -> window (frameNote.ts readFrame)
// - esc: an esc the page didn't use, frame -> window (guard.ts ESC_FORWARD; ArtifactView.tsx)
// - zoom: {fit}, an image's zoom left or reached fit, frame -> window (useFrameNote.ts claims esc
//   and ⌘0 while fit is false)
// - fit: back to fit, window -> frame (kit/office/image.js)
// - size: {h}, the height that holds the viewer's content, frame -> window (kit/office/video.js;
//   the card hugs it: useFrameNote.ts, designer m_32762)
// - pinch: {dy, x, y, w}, a trackpad pinch the page didn't use, frame -> window (guard.ts injects
//   main/pinch.ts PINCH_FORWARD; useFrameNote.ts zooms the frame, artifacts/pagezoom.ts)
// - pan: {dx, dy, w}, two fingers while the page is zoomed with room that way, frame -> window
//   (PINCH_FORWARD; useFrameNote.ts)
// - room: {room}, how far the zoomed page can still pan each way, null at fit, window -> frame
//   (useFrameNote.ts; PINCH_FORWARD takes it only from its parent)
// - storage: {local}, a site copy's localStorage, site frame -> bise's viewer (kit/storage.js ->
//   kit/office/site.js -> the hub; web-storage). The viewer takes it: the window never reads it.
// - find: {q, at, seq}, ⌘F's needle and the current match (-1: none), window -> frame, and a
//   frame -> its own frames (main/findagent.ts FIND_AGENT, artifacts/FindBar.tsx)
// - found: {seq, n}, how many matches the frame and its own frames hold, frame -> parent (FIND_AGENT)
export const FRAME_KINDS = { anchor: 'anchor', esc: 'esc', zoom: 'zoom', fit: 'fit', size: 'size', pinch: 'pinch', pan: 'pan', room: 'room', storage: 'site-storage', find: 'find', found: 'found' } as const;
