// bise kit · every message between an artifact's frame and the desktop window, by its `bise` kind
// (architect m_30630): one list, named on both sides. The desktop's copy is FRAME_KINDS in
// apps/desktop/src/artifacts/frameNote.ts; a law there reads this file and checks the two are
// equal, so a kind added on one side only is red.
//
// - anchor: a selection, frame -> window (select.js message(); frameNote.ts readFrameNote)
// - esc: an esc the page didn't use, frame -> window (the desktop's injected guard.ts ESC_FORWARD)
// - zoom: {fit}, an image's zoom left or reached fit, frame -> window (image.js; the window claims
//   esc and ⌘0 while fit is false)
// - fit: back to fit, window -> frame (image.js takes it only from window.parent)
// - size: {h}, the height that holds the viewer's content, frame -> window (video.js: the screen
//   at the clip's ratio and its bar; the window's card hugs it, designer m_32762)
// - pinch: {dy, x, y, w}, a trackpad pinch the page didn't use, frame -> window (the desktop's
//   injected main/pinch.ts PINCH_FORWARD; the window zooms the frame itself, useFrameNote.ts)
// - pan: {dx, dy, w}, two fingers while the page is zoomed with room that way, frame -> window
//   (PINCH_FORWARD; useFrameNote.ts)
// - room: {room}, how far the zoomed page can still pan each way, null at fit, window -> frame
//   (PINCH_FORWARD takes it only from window.parent)
// - storage: {local}, a site copy's whole localStorage after a write, site frame -> bise's viewer
//   (kit/storage.js, which writes the kind as a literal; office/site.js sends it to the hub,
//   web-storage, architect m_46151). Never reaches the window: the viewer takes it.
// - find: {q, at, seq}, ⌘F's needle and the current match (-1: none), window -> frame and a frame
//   -> its own frames (the desktop's injected main/findagent.ts FIND_AGENT; artifact-find)
// - found: {seq, n}, how many matches a frame and its own frames hold, frame -> parent (FIND_AGENT)

export const FRAME = Object.freeze({ anchor: "anchor", esc: "esc", zoom: "zoom", fit: "fit", size: "size", pinch: "pinch", pan: "pan", room: "room", storage: "site-storage", find: "find", found: "found" });
