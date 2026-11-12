//! The shell put together from its parts (moved out of `run`, so the
//! loop budget's tests build the same shell the hub runs:
//! daemon/loop_budget_tests.rs).

use super::*;

/// What `run` made before the shell: its options, its sb-core, its
/// channel's sender, the journal's writer and the photos' bookkeeping.
pub(super) struct Parts {
    pub(super) opts: Opts,
    pub(super) hub: Hub,
    pub(super) tx: Sender<Msg>,
    pub(super) journal: journal_file::Out,
    pub(super) photos: photo_take::Photos,
    pub(super) proc_hub: String,
}

impl Shell {
    /// The shell of `parts`, its workers started (trash, wake look,
    /// place work, processes); nothing booted yet.
    pub(super) fn assemble(parts: Parts) -> Shell {
        let Parts { opts, hub, tx, journal, photos, proc_hub } = parts;
        let paths = opts.paths.clone();
        let (log_paths, trash_paths) = (paths.clone(), paths.clone());
        // a drop's folder rm on the trash's worker, off this loop (trash.rs)
        let trash = crate::trash::Trash::worker(Box::new(move |s| log_line(&trash_paths, s)));
        let env = GitEnv::new(&paths, Box::new(move |s| log_line(&log_paths, s)), trash);
        // the checker (approvals-design.md §4): a chat model in the role runs
        // through repl-live's one-shot, like the role lines
        let home = opts.home.clone();
        let user_config = std::fs::read_to_string(home.config_file()).unwrap_or_default();
        let runner = crate::approvals::check::Runner::new(&home).with_oneshot(opts.repl_bin.clone(), opts.app_root.clone(), opts.spawn_env);
        let spool = usage_spool::UsageSpool::new(&opts, &paths.workspace, now_ms());
        Shell {
            opts,
            // before `hub` moves: its split of each message (crate::spent)
            loop_watch: loop_watch::LoopWatch::start(&paths, hub.spent.clone()),
            hub,
            env,
            tx: tx.clone(),
            journal,
            repls: BTreeMap::new(),
            gens: BTreeMap::new(),
            next_gen: 1,
            pids: BTreeMap::new(),
            starts: repl_starts::new_starts(),
            start_queue: BTreeMap::new(),
            clients: BTreeMap::new(),
            replies: BTreeMap::new(),
            buffers: BTreeMap::new(),
            positions: BTreeMap::new(),
            proto: proto::Proto::default(),
            answers: answers::Answers::new(crate::paths::workspace_id(&paths.workspace)),
            rpc: rpc::Rpcs::default(),
            projects: projects::Projects::boot(&home, &paths),
            state_gate: state_gate::StateGate::default(),
            context_gate: state_gate::StateGate::every(crate::contexts::CONTEXT_MS),
            history: history_work::HistoryWork::start(),
            offsets: BTreeMap::new(),
            booting: false,
            bins: BTreeMap::new(),
            ports: BTreeMap::new(),
            switching: BTreeMap::new(),
            switch_spawned: BTreeSet::new(),
            restored: BTreeSet::new(),
            partials: BTreeMap::new(),
            tix: tindex::Indexes::default(),
            finds: BTreeMap::new(),
            building: BTreeSet::new(),
            updating: None,
            release: None,
            update_told: None,
            release_switched: None,
            resume_turn: BTreeSet::new(),
            recorders: BTreeMap::new(),
            reload_id: String::new(),
            reload_repls: BTreeSet::new(),
            spawn_keys: BTreeMap::new(),
            spawn_plugins: BTreeMap::new(),
            tick: tick::Clock::new(now_ms()),
            recycle: recycling::Recycle::from_env(),
            usage: Default::default(),
            usage_spool: spool,
            pulse: Default::default(),
            small_broken: Default::default(),
            config: config_watch::ConfigWatch::new(&home, &paths),
            archived: BTreeSet::new(),
            proc_hub: proc_hub.clone(),
            down: BTreeSet::new(),
            gates: gate::Gates::new(&user_config, std::env::var(crate::approvals::mode::ENV).ok().as_deref(), runner),
            views: views::Views::default(),
            apps: apps::Apps::default(),
            files: view_files::DocFiles::default(),
            ids: files_ids::Ids::default(),
            lands: crate::land::Queue::default(),
            features: features::Features::load(&paths.state, &|n| paths.feature_diff(n)),
            prs: None,
            commit_style: None,
            cu: crate::computer_use::Watch::new(&proc_hub),
            disk: disk_watch::DiskWatch::from_env(),
            pg: pages::PageState::load(&paths.state),
            fn_ctx: None,
            idle: crate::idle::Watch::new(
                crate::idle::grace(std::env::var(crate::idle::ENV).ok().as_deref(), &user_config),
                crate::idle::waits(&user_config),
                std::time::Instant::now(),
            ),
            holds: crate::idle::Holds::default(),
            art: art::Art::default(),
            guides: guide::Guides::default(),
            revive: revive::Revive::default(),
            wake_look: wake_look::WakeLook::start(tx.clone()),
            place_work: place_work::PlaceWork::start(&paths, tx.clone()),
            procs: processes::Worker::start(paths.clone(), proc_hub.clone(), crate::paths::workspace_id(&paths.workspace), tx),
            photos,
        }
    }
}
