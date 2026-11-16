// src/inbox: one inbox (bar ⛔5, O.1; designer m_19631). Owner: amb-home.
// Block.tsx is the inbox: the block at the bottom of every view, every
// project's items, opened in place by ⌘I, /inbox, a notification or the
// capsule (openInbox); there is no inbox screen. InboxCard.tsx is one item as
// a card (the capsule's bottom cards mount it alone, amb-web); model.ts is its
// state and keys, order.ts its order (the hub's rank), stripRows.ts the
// block's rows and keys.
export { InboxCard, kindTitle, titleOf, PICKED_MS, type InboxCardActs, type InboxCardProps } from './InboxCard';
export { sortCards, cardKey } from './order';
export { takesWords, pickWaits } from './model';
export { InboxBlock, openInbox, inboxItemOpen, leaveClosed, answeringOf, closedAnswering, useNotifyAuth, useWaitingCount, type Answering, type Here, type InboxBlockProps, type InboxFold } from './Block';
export { blockList } from './listed';
export { cardDraftKey } from './drafts';
export { inboxOf } from './screen';
export { HubNoteRow } from './confirm';
