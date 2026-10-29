//! The views area's UI words (MCP Apps, docs/mcp-apps.md C7): a tool's
//! view card under its tool row, its panel tab, and their states (see
//! [`super`]: the rules, how to read them). The desktop's `src/views/`
//! draws them; the choices (which word for which state) stay there.

crate::words! { views:
    // ---- the card's states
    /// a view while its page loads (the card's quiet line, before the view said it is ready)
    loading = "loading the view…";
    /// the view's stored result is gone (the daily prune took it): the card keeps the tool's text
    expired = "this view expired";
    /// the hub could not give the view (its plugin or server is gone, or another refusal)
    failed = "the view didn't load";
    /// the view's server went away while it ran (a call answered server-restarted or plugin-off)
    server_gone = "its server is gone";
    /// the tool's result was over the 4 MiB the hub keeps for a view
    too_large = "result too large for the view";
    /// the view runs without the permissions it declared (not granted at the plugin's add, C8)
    no_grant = "runs without its permissions: not granted when the plugin was added";
    /// the card while its view shows in the panel (fullscreen): the line that brings it back
    in_panel = "open in the panel";
    /// the card's button that opens the view in the panel
    open_panel = "open in the panel";
    /// the card's button that brings a panel view back under its tool row
    back_inline = "show here";
    /// try again after a load that failed
    retry = "try again";

    // ---- the tab
    /// a view's panel tab title with no tool name yet
    tab_view = "view";

    // ---- what a view asked (the card's footnote, one at a time)
    /// the view asked the agent something: it went 'via <plugin>'
    message_sent(plugin) = "sent to the agent via {plugin}";
}
