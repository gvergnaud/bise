//! An older home hub (before client-protocol: it doesn't serve
//! `initialize`, architect m_15476): the core moves that hub to its own
//! install (proto-zone-c's part, next); until then the window says the
//! hub refused it, once per connection.

use super::*;

impl Core {
    /// The home hub answered `initialize` the older way.
    pub(super) fn home_older(&mut self) {
        self.hub_up = Some(false);
        let project = bise_home::hub_id(Path::new(&self.workspace));
        self.emit(json!({"ev": "hub_refused", "project": project, "error": "bise runs an older hub than this app"}));
    }
}
