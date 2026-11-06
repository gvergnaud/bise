// The one door from the feature-test helpers (inputs.ts, run.ts) into the app: the proto's
// generated types and fixtures, the store (until state-machine's step replaces it, fast-try p4),
// main's kept state events (a reload's replay) and each screen's own "what I show" function.
// A feature test never imports this (eslint): it reads only run.ts's views and sent commands.
export type { AnswerResult, Artifact, Card, Folder, CoreEv, HubEv, Mode, Part, ProjectRow, SendReason } from '@/proto';
export type { CoreCmd } from '@/shared/api';
export { core_ev, hub_ev } from '@/proto/fixtures.gen';
export { Store, threadKey, type AnswerFrom, type Pending } from '@/store';
export { Latest } from '@/shared/latest';
// the screens' views: the store's own (architect m_29407, m_29221 #2), the ones their components draw from
export type { InboxItem } from '@/inbox/listed';
export type { Effect } from '@/store/machine/types';
export { allItems } from '@/artifacts/hooks';
export { galleryView, type CardView, type Place } from '@/artifacts/gallery';
export { libraryView, type LibraryView } from '@/artifacts/folders';
// the window's route after a move (an option's nav effect, a click on an artifact's name) and what the sidebar shows for it
export type { Route } from '@/app/route';
export { askEffect, navRoute } from '@/app/edge';
// the capsule's card (the spotlight's waiting row): its gestures and its edge, which asks the main window
export { capsuleCardActs, runCapsuleEffect } from '@/capsule/cardacts';
export { artifactGo } from '@/artifacts/leave';
// a card's chip (and ⌘O): the intent the inbox block does (inbox/Block.tsx)
export { chipOpen } from '@/inbox/about';
export { libraryPage, sideShows, type SideShows } from '@/app/sideshows';
export { projectsOf, projectRowOf } from '@/app/siderows';
export { projectWaiting } from '@/inbox/waiting';
export { rowBars } from '@/app/projectfacts';
// an artifact the list doesn't have: what the centre and the panel show (artifacts/gone.ts)
export { centreOf, goneLinkOf, goneRoute, type ArtCentre, type GoneLink } from '@/artifacts/gone';
// the outer card's width by its border (side3d/outerwidth.ts) on the window's keep, and the layout that draws it
export { createKeep, type KeepStore } from '@/app/keep';
// the rail's places, and the bise tab's way back to his last project or chat on that keep (app/railback.ts)
export { createBiseBack, RAIL_GO, type BiseBack, type RailPlace } from '@/app/railback';
export { createOuterWidth, type OuterWidth } from '@/app/side3d/outerwidth';
export { sideLayout, type CoinsAt } from '@/app/side3d/layout';
export { leftWidthOf } from '@/app/frame-state';
export { SIDE } from '@/app/resize';
// the tabbed panel's own pure steps (panel/model.ts) and the page band beside the right side (app/pagecol.ts)
export { FOLDED, boxOf, closeBackKept, closeKept, fitOf, openKept, type Before, type PanelKept } from '@/app/panel/model';
export { THREAD_KEEPS, sideShows as rightSideShows } from '@/app/side3d/layout';
export { pageBand } from '@/app/pagecol';
