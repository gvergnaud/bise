// src/ds · bise desktop's design system: desktop-ds v5 (copied in from designer's reference; src/ds
// is the design system now) as CSS-variable tokens (tokens.css) and React components. Owner: amb-kit.
//
// The rules every component keeps:
// - a pure render of its props: no state, no effect, no store; the screens own state and keys;
// - the reference's class names and markup, so css/ stays the reference's ds.css, untouched
//   (live overrides only in css/live.css);
// - what he can act on takes a `testId` (data-testid `<area>.<thing>[.<id>]`, chosen by the screen;
//   list parts get `<testId>.<id>`) and a handler; never a test hook;
// - slots, not inputs: the composer's editor, the spotlight's input and the live pearl are other
//   chunks' (src/composer, src/spotlight, src/pearl) and go in as children, never re-created here;
// - focus and hover show through real selectors (:focus-within, :hover), so typing re-renders nothing;
// - rows that sit in long lists (Row, Msg, SpotRow, MenuItem, Job) are memo'd and take a ref.
// The gallery (gallery/Gallery.tsx) draws every component in each state, dark and light.
export { cx, st, step5, sub, type Cls, type UiState } from "./cx";
export * from "./prim";
export { holdCover, useCovered, useCoverWhile } from "./cover";
export * from "./window";
export * from "./composer";
export * from "./thread";
export * from "./overlays";
export * from "./panels";
export * from "./controls";
export * from "./mark";
export * from "./tip";
export * from "./picker";
export * from "./dropdown";
export * from "./gust";
export * from "./slide";
export { textWidth } from "./hooks";
export * from "./ring";
export * from "./icons";
export * from "./icons-app";
export * from "./banner";
export * from "./case";
export * from "./filters";
export * from "./listrow";
export * from "./more";
