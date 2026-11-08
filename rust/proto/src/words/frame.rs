//! The frame area's UI words (see [`super`]: the rules, how to read them):
//! the desktop window's frame since bise fait mac (page native-mac §6,
//! mockup 1): its bar, its rail, its sidebar (apps/desktop/src/app's
//! Frame.tsx, SideRail.tsx, Sidebar.tsx, newagent.tsx). The places' own
//! names (inbox, scheduled, artifacts, processes, plugins, bise) are `panel`'s, read
//! there. Sentence case as the mockups say them; designer signs them.

crate::words! { frame:
    /// the sidebar's first row (⌘N, his m_44225): add a project (the
    /// add-a-project sheet)
    new_project = "New project";
    /// the sidebar's second row and the new agent sheet's head (⇧⌘N):
    /// start a new agent in the project in view
    new_agent = "New agent";
    /// the new agent sheet's head, under it: the project
    new_agent_in(project) = "in {project}";
    /// the sidebar's second row: the agent palette (⌘K)
    search = "Search";
    /// the sidebar's label over the agents he pinned
    pinned = "Pinned";
    /// the sidebar's label over his projects; on the artifacts library, the
    /// row that goes back to them ('← Projects')
    projects = "Projects";
    /// the rail's VoiceOver name
    places = "Places";
    /// a control's hover tip with its key ('inbox · ⌘I')
    tip_key(what, key) = "{what} · {key}";
    /// the bar's subtitle fact on a thread: its agent works, its turn's age (designer m_43892)
    bar_working(age) = "working · {age}";
    /// the bar's subtitle fact on a thread: its agent waits on him (after the ask icon, in the accent)
    bar_waits = "waits for you";
    /// the bar's sidebar button, its VoiceOver name while the sidebar shows
    hide_sidebar = "Hide sidebar";
    /// the same button while the sidebar is hidden
    show_sidebar = "Show sidebar";
    /// the bar's right-side button, left of the panel's, while the outer card shows: the coins in
    /// its place (side-mini, designer m_46706)
    side_compact = "Compact side";
    /// the same button while he picked the coins
    side_card = "Show side card";
    /// the same button dimmed: the window too narrow for the card
    side_no_room = "No room for the side card at this width";
    /// the bar's back arrow (⌘[)
    back = "Back";
    /// the bar's forward arrow (⌘])
    forward = "Forward";
    /// the rail's last button: settings (⌘,)
    settings = "Settings";
    /// the rail's inbox button: its VoiceOver name and tip (chrome: sentence case, designer m_43052)
    inbox = "Inbox";
    /// the rail's scheduled button
    scheduled = "Scheduled";
    /// the rail's artifacts button
    artifacts = "Artifacts";
    /// the rail's processes button (the page of what his agents left running)
    processes = "Processes";
    /// the rail's plugins button
    plugins = "Plugins";
    /// the '+' at the Projects label's end: its VoiceOver name and tip
    add_project = "Add a project";
    /// the sidebar border's VoiceOver name (drag it, or ←/→)
    sidebar_width = "Sidebar width";
}
