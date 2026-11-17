// The spotlight's thread pane opening and closing, the page's half (capsule-morph, his ask
// m_66232, designer m_66237). Main glides the window re-centred (main/morph.ts: 220 ms open,
// 160 ms close, --ease-out); the page lays out its end state at once and plays its part on the
// same t=0:
//   open   0 ms     the line between the two is there, the follow-up field has the keys
//          80-220   the pane's content fades in and slides 12 px from the left (capsule.css)
//          120-260  his message rises 8 px and fades in; bise's ∿ right after (260-400)
//   close  0-80     the pane's content fades out (PANE_LEAVE_MS), then it closes and the
//                   window glides back to a centred spotlight in 160 ms
// Reduced motion: none of it. The frame jumps to its end and the content is simply there.

/** The pane's content fades out this long before it closes. */
export const PANE_LEAVE_MS = 80;

/** The open's entrance plays this long (its last part ends at 400 ms), then its rows play no more. */
export const PANE_ENTER_MS = 600;

/** He asked macOS for reduced motion: the pane opens and closes with no motion at all. */
export const stillMotion = (): boolean => typeof window !== "undefined" && typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
