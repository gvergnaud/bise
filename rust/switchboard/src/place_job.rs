//! A worktree job of sb-core's (hub-loop, docs/issues/32): its fx
//! `worktree_create`, `worktree_drop` and `worktree_restore`
//! (bend/hub/place.bend), run off the
//! hub's loop by daemon/place_work.rs. Pure data; the decision is sb-core's.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum Job {
    /// Make task `name`'s worktree (the shell picks its path and branch),
    /// from `feature`'s tip when not empty (`sb spawn --feature`), else
    /// main's; its end comes back as `Input::Placed`.
    Create { name: String, with_changes: bool, feature: String },
    /// Save what `dirty`/`unpushed` say would be lost, then remove the
    /// worktree `ws` of task `name`.
    Drop { name: String, ws: Value, dirty: u64, unpushed: u64 },
    /// Make the dropped worktree `ws` of task `name` again, its saved work
    /// (`snapshot`) put back; its end comes back as `Input::Placed`
    /// (issue 32 3b: a /restore off the loop).
    Restore { name: String, ws: Value, snapshot: Option<String> },
}

/// A create's job, the same for the worker and the tests: the made
/// worktree's ws (from `feature`'s tip when not empty), or git's words.
pub(crate) fn create(env: &mut dyn crate::core::Env, name: &str, with_changes: bool, feature: &str) -> (Value, String) {
    let made = if feature.is_empty() { env.worktree_create(name, with_changes) } else { env.worktree_feature(name, feature) };
    match made {
        Ok(w) => (serde_json::to_value(w).unwrap_or(Value::Null), String::new()),
        Err(e) => (Value::Null, e),
    }
}

/// A restore's job, the same for the worker and the tests: the made
/// worktree's ws, or git's words.
pub(crate) fn restore(env: &mut dyn crate::core::Env, name: &str, ws: &Value, snapshot: Option<&str>) -> (Value, String) {
    match serde_json::from_value(ws.clone()).map_err(|e| format!("its workspace does not read: {e}")).and_then(|w| env.worktree_restore(name, &w, snapshot)) {
        Ok(w) => (serde_json::to_value(w).unwrap_or(Value::Null), String::new()),
        Err(e) => (Value::Null, e),
    }
}

/// Tests: what daemon/place_work.rs answers a `worktree_create`, at once
/// (the env's worktree), as the `Input::Placed` the loop would step.
#[cfg(test)]
pub(crate) fn placed_now(env: &mut dyn crate::core::Env, e: &crate::core::Effect) -> Option<crate::core::Input> {
    use crate::core::Input;
    let crate::core::Effect::Place(job) = e else { return None };
    Some(match job {
        Job::Create { name, with_changes, feature } => {
            let (ws, err) = create(env, name, *with_changes, feature);
            Input::Placed { name: name.clone(), ws, err }
        }
        // issue 32 3b: a drop's end, as the worker sends it
        Job::Drop { name, ws, dirty, unpushed } => {
            let loss = crate::core::Loss { dirty: *dirty as usize, unpushed: *unpushed as usize };
            match serde_json::from_value(ws.clone()).map_err(|e| e.to_string()).and_then(|w| env.worktree_drop(name, &w, &loss)) {
                Ok(snapshot) => Input::Unplaced { name: name.clone(), snapshot, err: String::new() },
                Err(err) => Input::Unplaced { name: name.clone(), snapshot: None, err },
            }
        }
        Job::Restore { name, ws, snapshot } => {
            let (ws, err) = restore(env, name, ws, snapshot.as_deref());
            Input::Placed { name: name.clone(), ws, err }
        }
    })
}
