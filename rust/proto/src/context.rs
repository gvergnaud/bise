//! The fn context (architect's decision F): what the Mac app read of the
//! front app when the user pressed fn, sent with his words. One typed
//! struct for every reader: the home hub's route guess
//! (switchboard's route.rs), the routed message to a project's main, the
//! transcript's render. Every field is optional (an excluded app sends
//! nothing, a terminal has no URL) and left out of the JSON when absent.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FnContext {
    /// the front app's name (`Safari`, `Code`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// its page's URL, for a browser
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// its window's title
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// the file it shows, an absolute path, for an editor or a terminal
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// the text the user selected
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<String>,
    /// the window's visible text (Accessibility), cut by the app
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_text: Option<String>,
    /// the screen capture taken at fn: a path to the PNG the shell wrote
    /// (the window draws it as a thumbnail on his message, amb-win m_8492)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shot: Option<String>,
}
