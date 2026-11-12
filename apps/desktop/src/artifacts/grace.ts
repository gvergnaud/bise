// src/artifacts · the loading grace (artifact-loading, his m_60703), out of reach.ts so a place with
// no React (the views store, built into the web host too) reads the same numbers: a page or a
// document's view that doesn't answer yet inside LOAD_GRACE_MS is still loading, asked again every
// LOAD_RETRY_MS; only past it does he read an error. Pure. Owner: artifact-loading.

/**
 * How long a page that doesn't answer yet stays loading before the panel says it doesn't answer:
 * a server that refuses the first asks, or takes over the probe's 800 ms, is a hub or pages server
 * still starting, not a failure. The error showed at once and went away with the next ask, a few
 * seconds of a false 'nothing answers at 127.0.0.1:<port>'.
 */
export const LOAD_GRACE_MS = 10_000;
/** How often a page that doesn't answer yet is asked again inside LOAD_GRACE_MS. */
export const LOAD_RETRY_MS = 500;
