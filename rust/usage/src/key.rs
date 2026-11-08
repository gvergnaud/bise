//! The closed keys of the usage counts. A process counts only under one of
//! these: never a key made of a string it read (a command's argument, a
//! name, a path). A new metric is a new value here, so it changes the
//! exported schema (`schema.json`) and `docs/telemetry.md` in the same
//! commit (the snapshot test fails until both follow).
//!
//! Three kinds, three enums, so a count can't be added as minutes:
//! [`CountKey`] (how many times, capped 999), [`MinutesKey`] (how long,
//! kept in seconds by a process, sent in minutes rounded to 5) and
//! [`HistKey`] (how many observations fell in each fixed bucket). Plus the
//! one-time [`Milestone`]s and their [`Delay`].

closed! {
    /// How many times something happened in the day.
    pub enum CountKey {
        /// An agent created by him (the TUI, the app): task_created, origin the user.
        AgentCreatedByUser = "agent_created_by_user",
        /// An agent created by main.
        AgentCreatedByMain = "agent_created_by_main",
        /// An agent created by another agent.
        AgentCreatedByAgent = "agent_created_by_agent",
        /// A message he wrote to an agent (counts the day as active).
        UserMessage = "user_message",
        /// A card opened in his inbox.
        CardOpened = "card_opened",
        /// A card he answered (counts the day as active).
        CardAnswered = "card_answered",
        /// A turn that ended normally.
        TurnDone = "turn_done",
        /// A turn that failed (a provider error).
        TurnFailed = "turn_failed",
        /// A turn that stalled.
        TurnStalled = "turn_stalled",
        /// A compaction of an agent's context.
        Compaction = "compaction",
        /// A provider call retried.
        ProviderRetry = "provider_retry",
        /// A schedule set (`sb every`).
        ScheduleSet = "schedule_set",
        /// A schedule that fired.
        ScheduleFired = "schedule_fired",
        /// A trigger set (`sb wake`).
        TriggerSet = "trigger_set",
        /// A trigger that fired.
        TriggerFired = "trigger_fired",
        /// A page published.
        PagePublished = "page_published",
        /// A worktree placed for a task.
        WorktreePlaced = "worktree_placed",
        /// A land on main.
        Land = "land",
        /// His inbox opened in the app.
        InboxOpened = "inbox_opened",
        /// A page opened (an html artifact, a bise page).
        ArtifactOpenedPage = "artifact_opened_page",
        /// A document opened (markdown, pdf, docx...).
        ArtifactOpenedDoc = "artifact_opened_doc",
        /// A spreadsheet opened.
        ArtifactOpenedSheet = "artifact_opened_sheet",
        /// A deck opened.
        ArtifactOpenedSlides = "artifact_opened_slides",
        /// An image opened.
        ArtifactOpenedImage = "artifact_opened_image",
        /// A link opened.
        ArtifactOpenedLink = "artifact_opened_link",
        /// Code opened (a source file, a diff).
        ArtifactOpenedCode = "artifact_opened_code",
        /// Any other artifact opened (a site, a video, a sound, a folder, a PR...).
        ArtifactOpenedOther = "artifact_opened_other",
        /// A plugin turned on.
        PluginEnabled = "plugin_enabled",
        /// A connector call.
        ConnectorCall = "connector_call",
        /// A voice conversation started.
        VoiceStarted = "voice_started",
        /// A dictation started.
        DictationStarted = "dictation_started",
        /// A computer-use action.
        ComputerUseCall = "computer_use_call",
        /// A search (spotlight, the palette, history).
        Search = "search",
        /// The setup started.
        SetupStarted = "setup_started",
        /// The setup ended with a working provider.
        SetupDone = "setup_done",
        /// The setup's provider check failed on its key.
        SetupFailedAuth = "setup_failed_auth",
        /// The setup's provider check failed on the network.
        SetupFailedNetwork = "setup_failed_network",
        /// The setup's provider check failed otherwise.
        SetupFailedOther = "setup_failed_other",
        /// A new TUI panic file in `crashes/`.
        CrashTui = "crash_tui",
        /// The app's renderer process went away.
        CrashAppRenderer = "crash_app_renderer",
        /// One of the app's child processes went away.
        CrashAppChild = "crash_app_child",
        /// The app's core exited without being asked.
        CrashCore = "crash_core",
        /// A hub started after one that did not stop cleanly.
        CrashHub = "crash_hub",
    }
}

closed! {
    /// How long something lasted in the day.
    pub enum MinutesKey {
        /// Agent time: the turns' durations, long silences taken out (agent-hours).
        AgentWork = "agent_work",
        /// The app open.
        AppOpen = "app_open",
        /// The app focused.
        AppFocused = "app_focused",
        /// The app focused, with his input in the last minutes.
        AppEngaged = "app_engaged",
        /// A TUI attached to a hub.
        TuiAttached = "tui_attached",
        /// A TUI focused.
        TuiFocused = "tui_focused",
    }
}

closed! {
    /// Observations sorted into fixed buckets.
    pub enum HistKey {
        /// The most agents working at once in a hub, one observation per hub and day.
        PeakAgents = "peak_agents",
    }
}

closed! {
    /// A first time, sent once with its delay since the first launch.
    pub enum Milestone {
        /// The setup ended with a working provider.
        Setup = "setup",
        /// His first agent.
        FirstAgent = "first_agent",
        /// His first artifact opened.
        FirstArtifact = "first_artifact",
        /// His first land.
        FirstLand = "first_land",
    }
}

closed! {
    /// How long after the first launch a milestone came.
    pub enum Delay {
        UnderHour = "under_1h",
        UnderDay = "under_1d",
        UnderWeek = "under_7d",
        Later = "later",
    }
}

/// The cap of every count and every bucket: 999 (a bigger number says
/// more about one person than about use).
pub const COUNT_MAX: u16 = 999;

/// Minutes are sent in steps of 5.
pub const MINUTE_STEP: u16 = 5;

impl MinutesKey {
    /// The most minutes a packet carries for it, the one place of this cap
    /// (round_minutes and the schema, so the PII law, read it): a day (1440)
    /// for a clock he lives by; agent time adds the agents up, so it gets
    /// ten agents' full day (14400).
    pub fn cap(self) -> u16 {
        match self {
            MinutesKey::AgentWork => 14_400,
            MinutesKey::AppOpen | MinutesKey::AppFocused | MinutesKey::AppEngaged | MinutesKey::TuiAttached | MinutesKey::TuiFocused => 1440,
        }
    }
}

impl HistKey {
    /// The buckets' lower bounds, increasing; the last one is open.
    pub fn bounds(self) -> &'static [u32] {
        match self {
            HistKey::PeakAgents => &[1, 2, 3, 5, 9, 17],
        }
    }

    /// The bucket of a value, None below the first bound.
    pub fn bucket(self, v: u32) -> Option<usize> {
        self.bounds().iter().rposition(|b| v >= *b)
    }

    /// The buckets' labels: `3-4`, the last `17+`.
    pub fn labels(self) -> Vec<String> {
        let b = self.bounds();
        (0..b.len())
            .map(|i| match b.get(i + 1) {
                Some(next) if *next == b[i] + 1 => b[i].to_string(),
                Some(next) => format!("{}-{}", b[i], next - 1),
                None => format!("{}+", b[i]),
            })
            .collect()
    }
}

impl Delay {
    /// The bucket of a delay in seconds.
    pub fn of_secs(secs: u64) -> Delay {
        match secs {
            s if s < 3600 => Delay::UnderHour,
            s if s < 86_400 => Delay::UnderDay,
            s if s < 7 * 86_400 => Delay::UnderWeek,
            _ => Delay::Later,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_lands_in_its_bucket() {
        let k = HistKey::PeakAgents;
        assert_eq!(k.labels(), ["1", "2", "3-4", "5-8", "9-16", "17+"]);
        assert_eq!([0, 1, 2, 4, 5, 16, 17, 400].map(|v| k.bucket(v)), [None, Some(0), Some(1), Some(2), Some(3), Some(4), Some(5), Some(5)]);
    }

    #[test]
    fn names_are_unique_snake_case() {
        let mut names: Vec<&str> = CountKey::ALL.iter().map(|k| k.name()).collect();
        names.extend(MinutesKey::ALL.iter().map(|k| k.name()));
        names.extend(HistKey::ALL.iter().map(|k| k.name()));
        let n = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), n, "a key name is used twice");
        assert!(names.iter().all(|s| s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')));
        assert_eq!(CountKey::from_name("land"), Some(CountKey::Land));
        assert_eq!(CountKey::from_name("/Users/camille"), None);
    }

    #[test]
    fn delays_bucket_at_hour_day_week() {
        assert_eq!([0, 3599, 3600, 86_399, 86_400, 604_799, 604_800].map(Delay::of_secs), [
            Delay::UnderHour,
            Delay::UnderHour,
            Delay::UnderDay,
            Delay::UnderDay,
            Delay::UnderWeek,
            Delay::UnderWeek,
            Delay::Later
        ]);
    }
}
