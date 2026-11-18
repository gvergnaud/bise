// A link's right click in a thread (links-os, designer m_66370): the thread (ThreadRows) and the
// agent's peek give Markdown the menu to show (app/panel/web.ts openLinkMenu); elsewhere (the
// capsule, spotlight, a skill's text) none, and a right click on a link does what the system does.
// Owner: links-os.
import { createContext } from 'react';

/** The menu a right click on a link shows, given its URL; null: none here. */
export const LinkMenuContext = createContext<((url: string) => void) | null>(null);
