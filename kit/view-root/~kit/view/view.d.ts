//! Types only, never served: what `./~kit/view/view.js` is to tsc in a first-party view, through
//! its tsconfig's rootDirs ["ai.mistral.vibe/views/<v>", "<repo>/kit/view-root"], so tsc resolves
//! the same relative path the browser does (the hub serves ~kit/view/ from kit/view/, never this
//! folder: rust/switchboard/src/pages/view_folder.rs). Kept in step with kit/view's public surface.
export * from "../../../view/view.js";
