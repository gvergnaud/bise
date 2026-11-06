//! The hub's side of an agent's `--attach` (send-parts, architect
//! m_23962): `sb send` and `sb spawn` with attachments become a message
//! with parts, the model's text their render. The decisions are
//! `crate::attach`'s (pure); here, only the file system:
//!
//! - an image is put in the image store by `bend_images::store_file`
//!   (it sniffs: a non-image is refused), the one read the hub makes for
//!   an agent; a `.b64` of the store goes through the image beside it;
//! - a file is never read: it must exist, and goes by its path; one under
//!   the sender's `$TMPDIR` is copied to the receiver's (`sb send` only:
//!   `sb spawn` is main's, whose tmp lives as long as the hub);
//! - `m_<n>`: the images and files of a message the sender got or sent,
//!   from the hub's messages (`State::msgs`);
//! - then `parts_in::said`'s checks, as for his own parts (an unknown
//!   part, an image outside the store: refused).
//!
//! The CLI already opened every path in the sender's sandbox. The request's
//! `parts` is set here only, never read from the CLI.

use super::*;
use crate::attach::{self, Item, Kind, Want};
use crate::core::AgentReq;
use std::path::Path;

impl Shell {
    /// `req` with the parts its `attach` list makes (`v`: the CLI's
    /// request); none: unchanged. Err: the words of the refusal.
    pub(super) fn attached(&mut self, from: &str, req: AgentReq) -> Result<AgentReq, String> {
        // view-all-perf: an `m_<n>` attached is read whole: a cold one's
        // body first
        if let AgentReq::Send { attach, .. } | AgentReq::Spawn { attach, .. } = &req {
            let ids: Vec<u64> = attach.iter().filter_map(|a| crate::core::parse_msg_id(a)).collect();
            self.hub.fetch_bodies(ids);
        }
        let sender = || self.hub.st.resolve(from).unwrap_or_else(|| from.to_string());
        match req {
            AgentReq::Send { to, text, expect_reply, reply_to, queued, why, switch, attach, fwd, .. } if !attach.is_empty() => {
                attach::capped(attach.len())?;
                let sender = sender();
                let tmps = self.hub.st.resolve(&to).and_then(|r| {
                    let dir = |n: &str| self.hub.st.agents.get(n).map(|a| self.opts.paths.agent_tmp(&a.dir));
                    Some((dir(&sender)?, dir(&r)?))
                });
                let items = self.items(&sender, &attach, tmps.as_ref())?;
                let said = said(&text, &items)?;
                Ok(AgentReq::Send { to, text: said.text, expect_reply, reply_to, queued, why, switch, attach, parts: said.parts, fwd })
            }
            AgentReq::Spawn { name, brief, worktree, with_changes, place, feature, ask, attach, fwd, .. } if !attach.is_empty() => {
                attach::capped(attach.len())?;
                let items = self.items(&sender(), &attach, None)?;
                let said = said(&crate::prompts::brief_body(&brief), &items)?;
                Ok(AgentReq::Spawn { name, brief, worktree, with_changes, place, feature, ask, attach, parts: said.parts, fwd })
            }
            req => Ok(req),
        }
    }

    /// Each `--attach` found (`tmps`: the sender's and the receiver's
    /// tmp, when a file in the first goes to the second).
    fn items(&self, sender: &str, list: &[String], tmps: Option<&(std::path::PathBuf, std::path::PathBuf)>) -> Result<Vec<Item>, String> {
        let store = bise_home::Home::from_env().images_dir();
        let mut out = Vec::new();
        for a in list {
            match attach::want(a)? {
                Want::Msg(n) => {
                    // whole: `attached` fetched a cold one's body
                    let m = self.hub.st.msg(n).map(|m| attach::Said { from: &m.from, to: &m.to, text: &m.text, parts: &m.parts });
                    out.extend(attach::forwarded(sender, n, m)?);
                }
                Want::Path(p) => out.push(path_item(&p, &store, tmps)?),
            }
        }
        attach::capped(out.len())?;
        Ok(out)
    }
}

/// One path found: an image stored, or a file by its path (copied first
/// when it is in the sender's tmp).
fn path_item(p: &str, store: &Path, tmps: Option<&(std::path::PathBuf, std::path::PathBuf)>) -> Result<Item, String> {
    let image = |src: &Path| {
        let s = super::fn_context::store_image(src).map_err(|e| format!("--attach {p}: {e}"))?;
        Ok(Item::Image { source: src.to_string_lossy().into_owned(), mime: s.mime, b64: s.b64 })
    };
    match attach::kind(p, store) {
        Kind::StoreB64 => {
            let beside = bise_proto::parts::IMAGE_EXTS.iter().map(|e| Path::new(p).with_extension(e)).find(|s| s.is_file());
            image(&beside.ok_or_else(|| format!("--attach {p}: no image beside it in the store"))?)
        }
        Kind::Image => image(Path::new(p)),
        Kind::File => {
            if !Path::new(p).is_file() {
                return Err(format!("--attach {p}: no such file"));
            }
            match tmps.and_then(|(a, b)| attach::tmp_copy(p, a, b)) {
                Some(dst) => {
                    let dir = dst.parent().unwrap_or(Path::new("/"));
                    std::fs::create_dir_all(dir).and_then(|_| std::fs::copy(p, &dst)).map_err(|e| format!("--attach {p}: not copied to its tmp ({e})"))?;
                    Ok(Item::File { path: dst.to_string_lossy().into_owned() })
                }
                None => Ok(Item::File { path: p.to_string() }),
            }
        }
    }
}

/// The parts checked and rendered, as his own (`parts_in::said`).
fn said(text: &str, items: &[Item]) -> Result<crate::parts_in::Said, String> {
    let parts = attach::parts(text, items);
    // an agent's attachments are images and files only: no artifact note
    let known = |_: &str, _: u64| Err("an agent's message carries no note on an artifact".to_string());
    let door = crate::parts_in::Door { store: &bise_home::Home::from_env().images_dir(), put: &super::fn_context::store_image, exists: &super::fn_context::file_exists, known: &known };
    crate::parts_in::said(&parts, &door)
}
