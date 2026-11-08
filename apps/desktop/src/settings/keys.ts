// The app's key list (bar 3.5: ⌘/ and settings · keys): every key of the
// app in one table, with the meaning the TUI's key has (rust/tui/src/help.rs
// ROWS), ⌘ standing in for ctrl where a Mac app has its own gesture. Pure
// data; the words are bise_proto's (words::settings, issue 29). Owner:
// amb-home; a chunk that adds a key adds its row here (ask).
import { settings } from '@/proto/words.gen';

/** The keys the two sides show (the bar's tips and held keycaps, the View menu, this list); the
 *  aliases ⌃⌘S, ⌘\ and ⌥⌘0 work and show nowhere (designer m_45164). app/keys.ts decides them. */
export const FRAME_KEYS: Readonly<Record<'sidebar' | 'panel', string>> = { sidebar: '⌘B', panel: '⌥⌘B' };

/** The left rail's places and the sidebar's head rows, each its own key (his m_45482, designer
 *  m_45492 / m_45497): their tips, their held keycaps, the Go menu and this list. ⌘I stays the inbox
 *  block in place (b1/b2); the rail's inbox button is the page, ⇧⌘I. app/keys.ts decides them. */
export const PLACE_KEYS = {
  bise: '⌘0',
  inbox: '⇧⌘I',
  scheduled: '⌘Y',
  artifacts: '⇧⌘A',
  processes: '⇧⌘P',
  plugins: '⇧⌘X',
  settings: '⌘,',
  search: '⌘K',
  newProject: '⌘N',
} as const;

/** The artifacts library's rows (All artifacts, Recent, Unfiled, then the folders) take ⌘1 … ⌘9
 *  in its order, as the projects do (designer m_45492). */
export const LIBRARY_ROWS_MAX = 9;

export interface KeyRow {
  /** the keys, as drawn ("⌘1 … ⌘9") */
  readonly keys: string;
  readonly action: string;
  /** the TUI's key for the same action, when it differs */
  readonly tui?: string;
}

export interface KeySection {
  readonly id: string;
  readonly title: string;
  readonly rows: readonly KeyRow[];
}

export const KEY_SECTIONS: readonly KeySection[] = [
  {
    id: 'anywhere',
    title: settings.keys_anywhere,
    rows: [
      { keys: 'hold fn', action: settings.key_hold_fn },
      { keys: 'fn fn', action: settings.key_fn_fn },
      { keys: 'fn space', action: settings.key_fn_space },
    ],
  },
  {
    id: 'window',
    title: settings.keys_window,
    rows: [
      { keys: FRAME_KEYS.sidebar, action: settings.key_sidebar },
      { keys: FRAME_KEYS.panel, action: settings.key_panel },
      { keys: '⇧⌘T', action: settings.key_reopen_tab },
      { keys: PLACE_KEYS.bise, action: settings.key_bise },
      { keys: '⌘1 … ⌘9', action: settings.key_project },
      { keys: '⌥0 / ⌥1 … ⌥9', action: settings.key_main_or_agent, tui: '⌥ + 0…9' },
      { keys: PLACE_KEYS.newProject, action: settings.key_new_project },
      { keys: '⇧⌘N', action: settings.key_new_agent },
      { keys: PLACE_KEYS.search, action: settings.key_find_agent },
      { keys: '⌘[ / ⌘]', action: settings.key_back_forward },
      { keys: '⌘I', action: settings.key_inbox, tui: '/inbox, ctrl+1-9' },
      { keys: PLACE_KEYS.inbox, action: settings.key_inbox_page },
      { keys: PLACE_KEYS.scheduled, action: settings.key_scheduled },
      { keys: PLACE_KEYS.artifacts, action: settings.key_artifacts, tui: '/artifacts' },
      { keys: PLACE_KEYS.processes, action: settings.key_processes },
      { keys: PLACE_KEYS.plugins, action: settings.key_plugins },
      { keys: '⌘F', action: settings.key_find, tui: 'ctrl+f' },
      { keys: '⌘D', action: settings.key_diff, tui: '/diff' },
      { keys: '⌘⏎', action: settings.key_artifact_primary },
      { keys: '⌘E', action: settings.key_fold, tui: 'ctrl+o' },
      { keys: PLACE_KEYS.settings, action: settings.key_settings },
      { keys: '⌘/', action: settings.key_this_list, tui: '/shortcuts' },
    ],
  },
  {
    id: 'composer',
    title: settings.keys_composer,
    rows: [
      { keys: '⏎', action: settings.key_send },
      { keys: '⇧⏎', action: settings.key_new_line },
      { keys: 'tab', action: settings.key_queue },
      { keys: '↑', action: settings.key_last },
      { keys: 'ctrl+c', action: settings.key_stop_turn, tui: 'ctrl+c' },
      { keys: 'esc', action: settings.key_put_away },
      { keys: '⌘.', action: settings.key_stop_agent },
      { keys: '/', action: settings.key_commands },
      { keys: '@', action: settings.key_file },
      { keys: '$', action: settings.key_skill },
      { keys: '⌘V', action: settings.key_paste_chip },
      { keys: 'ctrl+r', action: settings.key_dictation },
    ],
  },
  {
    id: 'inbox',
    title: settings.keys_inbox,
    rows: [
      { keys: '↑ ↓', action: settings.key_prev_next },
      { keys: 'ctrl+p / ctrl+n', action: settings.key_prev_next_round },
      { keys: 'ctrl+o', action: settings.key_open_folded },
      { keys: '← →', action: settings.key_walk_options },
      { keys: '1 … 9', action: settings.key_pick_option },
      { keys: '⏎', action: settings.key_pick_or_send },
      { keys: '⌘⌫', action: settings.key_close_item, tui: 'ctrl+x' },
      { keys: 'esc', action: settings.key_back },
    ],
  },
];
