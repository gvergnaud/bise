//! The desktop's settings pages: the nav, general, voice, context and
//! privacy, quiet moments, the keys page and the keychain row
//! (apps/desktop/src/settings/pages.tsx).

crate::words! { settings:
    /// the settings window's sidebar, for VoiceOver (mockup 10)
    nav_label = "Settings";
    /// the nav's page: general
    page_general = "General";
    /// general's hero sentence (designer m_39021)
    page_general_sub = "how bise looks, reaches you and updates.";
    /// the nav's page: setup
    page_setup = "Setup";
    /// setup's hero sentence
    page_setup_sub = "what's set up and what's left: a model, the permissions, your first project.";
    /// the nav's page: voice
    page_voice = "Voice and fn";
    /// voice's hero sentence
    page_voice_sub = "how bise hears you and answers: hold fn to talk from any app, ctrl+r to dictate in the composer.";
    /// the nav's page: context and privacy
    page_privacy = "Context and privacy";
    /// privacy's hero sentence
    page_privacy_sub = "what bise sees when you hold fn, and what it never sends.";
    /// the nav's page: quiet moments
    page_quiet = "Quiet moments";
    /// quiet's hero sentence
    page_quiet_sub = "when bise keeps quiet: in a call, while you share your screen, in full screen.";
    /// the nav's page: models
    page_models = "Models";
    /// models' hero sentence
    page_models_sub = "the models your agents use, and how you pay for them: a plan, a key, or both.";
    /// the nav's page: approvals
    page_approvals = "Approvals";
    /// approvals' hero sentence
    page_approvals_sub = "what runs without asking, and what waits for your yes.";
    /// the nav's page: projects
    page_projects = "Projects";
    /// projects' hero sentence
    page_projects_sub = "the folders bise works in. each one has its own lead.";
    /// the nav's page: computer use
    page_computer = "Computer use";
    /// computer use's hero sentence
    page_computer_sub = "your browser and your Mac apps, for your agents: set it up once, then see what they did.";
    /// the nav's page: keys
    page_keys = "Shortcuts";
    /// keys' hero sentence
    page_keys_sub = "the keys that reach bise from any app, and in the window.";

    /// general: the macOS notifications row's label
    notify_label = "macOS notifications";
    /// notifications never asked
    notify_never = "questions can reach you when bise isn't in front";
    /// notifications never asked: its button
    notify_turn_on = "Turn on";
    /// notifications asked once, the answer in System Settings
    notify_asked = "macOS asked you; your answer is in System Settings";
    /// notifications on
    notify_on = "questions reach you when bise isn't in front";
    /// notifications on: its chip
    notify_on_chip = "on";
    /// notifications off in macOS
    notify_off = "off in macOS: questions wait in bise until you look";
    /// a button that opens macOS's System Settings (notifications, a denied permission)
    open_system_settings = "open System Settings";
    /// general: the theme's group (mockup 10)
    theme_label = "Appearance";
    /// general: the theme row (mockup 10)
    theme_row = "Theme";
    /// general: the notifications' group (mockup 10)
    notify_title = "Notifications";
    /// general: the theme row's line
    theme_sub = "light, dark, or as macOS";
    /// a theme: as macOS
    theme_system = "System";
    /// a theme: light
    theme_light = "Light";
    /// a theme: dark
    theme_dark = "Dark";

    /// voice: hold fn
    voice_fn_hold = "Hold fn to talk";
    /// voice: hold fn's line
    voice_fn_hold_sub = "your words go when you let go";
    /// voice: fn twice
    voice_fn_twice = "fn fn: hands-free";
    /// voice: fn twice's line
    voice_fn_twice_sub = "talk without holding; fn again stops";
    /// voice: the language row
    voice_language = "What bise hears";
    /// voice: the language row's line
    voice_language_sub = "the language of your voice";
    /// a voice language: let the model guess
    lang_auto = "Auto";
    /// a voice language: English
    lang_en = "English";
    /// a voice language: French
    lang_fr = "Français";

    /// quiet: the first group's title
    quiet_wait = "Notifications wait";
    /// quiet: during a call
    quiet_call = "During a call";
    /// quiet: during a call's line
    quiet_call_sub = "Zoom, Meet, FaceTime: the pearl blushes, nothing shows";
    /// quiet: during a share
    quiet_share = "While you share your screen";
    /// quiet: during a share's line
    quiet_share_sub = "never a notification during a share";
    /// quiet: full screen
    quiet_fullscreen = "In full screen";
    /// quiet: full screen's line
    quiet_fullscreen_sub = "Keynote, a game, a video";
    /// quiet: focus mode
    quiet_focus = "In focus mode";
    /// quiet: focus mode's line
    quiet_focus_sub = "follows macOS focus";
    /// quiet: the second group's title
    quiet_then = "Then";
    /// quiet: the batch row
    quiet_batch = "They come together";
    /// quiet: the batch row's line
    quiet_batch_sub = "“2 questions · 1 min”, one at a time";
    /// quiet: how often a batch comes
    quiet_batch_min = "At most every";
    /// quiet: how often's line
    quiet_batch_min_sub = "to group the rest";
    /// quiet: a batch's minutes
    minutes(n) = "{n} min";
    /// quiet: the away card
    quiet_away = "While you were away";
    /// quiet: the away card's line
    quiet_away_sub = "one notification when you open your Mac again";

    /// privacy: the first group's title
    privacy_hold = "When you hold fn";
    /// privacy: the screenshot
    privacy_shot = "A screenshot of the window in front";
    /// privacy: the screenshot's line
    privacy_shot_sub = "never the whole screen";
    /// privacy: the tab's URL
    privacy_url = "The tab's URL and title";
    /// privacy: the tab's URL's line
    privacy_url_sub = "Chrome, Safari, Arc";
    /// privacy: the open file
    privacy_file = "The open file and your selection";
    /// privacy: the open file's line
    privacy_file_sub = "VS Code, Cursor, Xcode, Zed";
    /// privacy: fn space reads his selection, the spotlight's ❝ chip (context.quote)
    privacy_quote = "Quote what you selected";
    /// privacy: the quote's line
    privacy_quote_sub = "fn space only: a ❝ chip you see and can remove";
    /// privacy: the window's text
    privacy_window_text = "The text of the window";
    /// privacy: the window's text's line
    privacy_window_text_sub = "mails, docs, tickets: bise sees what you see";
    /// privacy: the second group's title
    privacy_kept = "Kept";
    /// privacy: the images row (D14: every image of the store, designer m_23410)
    privacy_shots = "Images";
    /// privacy: the images row's line
    privacy_shots_sub = "pasted, from agents or from fn";
    /// privacy: how long images are kept
    days(n) = one "{n} day" | "{n} days";
    /// privacy: the excluded apps' group
    privacy_apps = "Never sent to an agent";
    /// privacy: the excluded apps' help
    privacy_apps_help = "fn reads nothing in these apps: no title, URL, file, selection, text or capture.";
    /// privacy: the row that adds an app
    privacy_another_app = "Another app";
    /// privacy: its button
    privacy_add = "Add…";
    /// privacy: the bundle id field
    privacy_app_id = "its bundle id (com.example.app), then ⏎";

    /// keys: the terminal's key, when it differs
    key_terminal(keys) = "terminal: {keys}";
    /// keys: the chip under the list
    keys_same = "/help and /shortcuts in the terminal list the same keys";

    /// keys page: the keychain group
    keychain_title = "Your keys and sign-ins";
    /// keys page: the keychain row
    keychain_label = "Keep them in the macOS keychain";
    /// keys page: the keychain switch's label
    keychain_switch = "keep your keys and sign-ins in the macOS keychain";
    /// keys page: the move under way
    keychain_moving = "moving them…";
    /// keys page: they are in the keychain
    keychain_on = "on: your keys and sign-ins are in the macOS keychain.";
    /// keys page: they are in files
    keychain_off = "off: they're in files in ~/.bise. same switch as /keychain in the terminal.";
    /// general: his own bim's group (page bim-pick): what his bim in the office and on its coin looks like
    me_label = "Your bim";
    /// general, your bim: the group's line
    me_sub = "how you look in the office and on your coin";
    /// your bim: the skin row
    me_skin = "Skin";
    /// your bim: the hair row
    me_hair = "Hair";
    /// your bim: the hair colour row
    me_color = "Hair colour";
    /// your bim: what it wears on its head
    me_wear = "On the head";
    /// your bim: a choice greyed out because the hair can't take it (its tip)
    me_wear_no = "not with this hair";
    /// your bim: the colour of what it wears on its head (cap, beanie, headset; designer m_48063)
    me_hat = "Accessory colour";
    /// your bim, accessory colour: the row's tip when its dots are greyed (nothing on the head)
    me_hat_no = "Nothing on the head to colour";
    /// your bim: a skin's tip, light to dark (1 to 6)
    me_skin_n(n) = "skin {n} of 6";
    /// a hair cut: today's short
    hair_short = "short";
    /// a hair cut
    hair_afro = "afro";
    /// a hair cut: a bun on top
    hair_topbun = "top bun";
    /// a hair cut
    hair_long = "long";
    /// a hair cut
    hair_ponytail = "ponytail";
    /// a hair cut
    hair_braids = "braids";
    /// a hair cut
    hair_bob = "bob";
    /// a hair cut: two buns
    hair_buns = "two buns";
    /// a hair cut
    hair_curls = "curls";
    /// a hair cut
    hair_pigtails = "pigtails";
    /// a hair cut
    hair_mohawk = "mohawk";
    /// a hair cut
    hair_spiky = "spiky";
    /// a hair colour (a tip: sentence case, designer m_48380)
    color_black = "Black";
    /// a hair colour
    color_dark = "Dark brown";
    /// a hair colour
    color_brown = "Brown";
    /// a hair colour
    color_auburn = "Auburn";
    /// a hair colour
    color_blond = "Blond";
    /// a hair colour
    color_lilac = "Lilac";
    /// an accessory colour (a tip: sentence case): today's, the default
    hat_sky = "Sky blue";
    /// an accessory colour
    hat_orange = "Orange";
    /// an accessory colour
    hat_green = "Green";
    /// an accessory colour
    hat_yellow = "Yellow";
    /// an accessory colour
    hat_lilac = "Lilac";
    /// an accessory colour
    hat_teal = "Teal";
    /// on the head: nothing
    wear_none = "nothing";
    /// on the head
    wear_cap = "cap";
    /// on the head
    wear_beanie = "beanie";
    /// on the head: a headset
    wear_headphones = "headset";
}

