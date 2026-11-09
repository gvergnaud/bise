//! Which browser profile a call goes to, and the profiles bise knows
//! (pure; the broker holds them and writes `profiles.json`).
//!
//! Like connector accounts (page multi-comptes, pick A): no `profile`
//! argument goes to the default; a `profile` (its Chrome name, its account
//! email or its dir) goes to that one, and an unknown one is refused with
//! the connected list, never replaced by the default. The default is the
//! one he chose in Settings; until he chooses, the first profile that ever
//! connected (kept, so it never moves with the hellos' order).

use serde_json::{json, Value};

use crate::profiles::Profile;

/// A profile's key: its browser (C4 key, "chrome") and its dir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    pub browser: String,
    pub dir: String,
}

impl Key {
    pub fn to_json(&self) -> Value {
        json!({"browser": self.browser, "dir": self.dir})
    }

    pub fn from_json(v: &Value) -> Option<Key> {
        let browser = v.get("browser")?.as_str().filter(|s| !s.is_empty())?.to_string();
        let dir = v.get("dir")?.as_str().filter(|s| !s.is_empty())?.to_string();
        Some(Key { browser, dir })
    }
}

/// A profile bise saw connected once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Known {
    pub browser: String,
    pub profile: Profile,
}

impl Known {
    pub fn key(&self) -> Key {
        Key { browser: self.browser.clone(), dir: self.profile.dir.clone() }
    }
}

/// `profiles.json`: the profiles seen, and the default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Book {
    pub default: Option<Key>,
    pub known: Vec<Known>,
}

impl Book {
    pub fn from_json(v: &Value) -> Book {
        let default = v.get("default").and_then(Key::from_json);
        let known = v
            .get("known")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|k| {
                let browser = k.get("browser")?.as_str().filter(|s| !s.is_empty())?.to_string();
                Some(Known { browser, profile: Profile::from_json(k)? })
            })
            .collect();
        Book { default, known }
    }

    pub fn to_json(&self) -> Value {
        let known: Vec<Value> = self
            .known
            .iter()
            .map(|k| {
                let mut v = k.profile.to_json();
                v["browser"] = json!(k.browser);
                v
            })
            .collect();
        let mut v = json!({"known": known});
        if let Some(d) = &self.default {
            v["default"] = d.to_json();
        }
        v
    }

    /// A profile said hello: known from now on (its name and email as it
    /// says them now), the default when there is none. True: changed.
    pub fn seen(&mut self, browser: &str, profile: &Profile) -> bool {
        let before = self.clone();
        let k = Known { browser: browser.to_string(), profile: profile.clone() };
        match self.known.iter_mut().find(|x| x.key() == k.key()) {
            Some(x) => *x = k.clone(),
            None => self.known.push(k.clone()),
        }
        if self.default.is_none() {
            self.default = Some(k.key());
        }
        *self != before
    }

    /// His choice in Settings: a known profile only.
    pub fn set_default(&mut self, key: &Key) -> Result<(), String> {
        if !self.known.iter().any(|k| &k.key() == key) {
            return Err(format!("no profile {} in {} is known", key.dir, key.browser));
        }
        self.default = Some(key.clone());
        Ok(())
    }

    /// He forgets one (Settings): out of the list; the default passes to
    /// the first one left. It comes back at its next hello.
    pub fn forget(&mut self, key: &Key) -> bool {
        let n = self.known.len();
        self.known.retain(|k| &k.key() != key);
        if self.default.as_ref() == Some(key) {
            self.default = self.known.first().map(Known::key);
        }
        n != self.known.len()
    }
}

/// A connected extension as the choice sees it.
#[derive(Clone, Debug)]
pub struct Live<'a> {
    pub link: u64,
    /// the browser's C4 key and name ("chrome", "Chrome")
    pub key: &'a str,
    pub name: &'a str,
    /// None: an extension that didn't say (an older one), or not found
    pub profile: Option<&'a Profile>,
}

impl Live<'_> {
    /// How an agent or he reads it: "Chrome · pro", "Chrome" (unknown).
    pub fn label(&self) -> String {
        match self.profile {
            Some(p) => format!("{} · {}", self.name, p.name),
            None => self.name.to_string(),
        }
    }
}

/// Why no link: the C1 code and its message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    pub code: &'static str,
    pub message: String,
    /// for `ambiguous`: the profiles that match
    pub candidates: Vec<String>,
}

fn refused(code: &'static str, message: String) -> Refused {
    Refused { code, message, candidates: Vec::new() }
}

/// The link `open` goes to. `browser`, `profile`: the agent's arguments;
/// `mine`: the link of the agent's own tabs, if any. `live` is never empty
/// (the broker says no_browser before).
pub fn pick(live: &[Live], book: &Book, browser: Option<&str>, profile: Option<&str>, mine: Option<u64>) -> Result<u64, Refused> {
    let list = |l: &[&Live]| l.iter().map(|x| x.label()).collect::<Vec<_>>().join(", ");
    let all: Vec<&Live> = live.iter().collect();
    let pool: Vec<&Live> = match browser.map(str::trim).filter(|b| !b.is_empty()) {
        Some(w) => {
            let p: Vec<&Live> = all.iter().copied().filter(|x| x.key.eq_ignore_ascii_case(w) || x.name.eq_ignore_ascii_case(w)).collect();
            if p.is_empty() {
                return Err(refused("no_browser", format!("{} isn't connected (connected: {}); use one of those or ask the user", w, list(&all))));
            }
            p
        }
        None => all.clone(),
    };
    if let Some(w) = profile.map(str::trim).filter(|w| !w.is_empty()) {
        let hits: Vec<&Live> = pool.iter().copied().filter(|x| x.profile.is_some_and(|p| p.named(w))).collect();
        return match hits.as_slice() {
            [one] => Ok(one.link),
            [] => {
                // known but not open: say so, he can open it
                let known = book.known.iter().find(|k| k.profile.named(w) && browser.is_none_or(|b| k.browser.eq_ignore_ascii_case(b)));
                let msg = match known {
                    Some(k) => format!("the profile {} isn't open (connected: {}); ask the user to open {} in it, or use a connected one", k.profile.name, list(&pool), name_of(&k.browser)),
                    None => format!("no profile {:?} is connected (connected: {}); use one of those or ask the user", w, list(&pool)),
                };
                Err(refused("no_browser", msg))
            }
            many => Err(Refused {
                code: "ambiguous",
                message: format!("{:?} names {} profiles; pass browser too", w, many.len()),
                candidates: many.iter().map(|x| x.label()).collect(),
            }),
        };
    }
    // no profile asked: the default, when it is connected
    let default = book.default.as_ref().and_then(|d| pool.iter().find(|x| x.key == d.browser && x.profile.is_some_and(|p| p.dir == d.dir)));
    if let Some(d) = default {
        return Ok(d.link);
    }
    if let [one] = pool.as_slice() {
        return Ok(one.link);
    }
    if let Some(m) = mine.filter(|m| pool.iter().any(|x| x.link == *m)) {
        return Ok(m);
    }
    Err(Refused {
        code: "ambiguous",
        message: format!("the default profile isn't open and {} are: pass profile (its name), or ask the user which", pool.len()),
        candidates: pool.iter().map(|x| x.label()).collect(),
    })
}

/// A browser's name from its key, for messages.
fn name_of(key: &str) -> &str {
    crate::browsers::by_key(key).map(|b| b.name).unwrap_or(key)
}

/// What agents see of a link's profile (open, tabs, status): its browser,
/// dir and name; never the email (a match key and a Settings line only).
pub fn for_agents(key: &str, p: &Profile) -> Value {
    json!({"browser": key, "dir": p.dir, "name": p.name})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prof(dir: &str, name: &str, email: &str) -> Profile {
        Profile { dir: dir.into(), name: name.into(), email: (!email.is_empty()).then(|| email.into()) }
    }

    struct World {
        pro: Profile,
        perso: Profile,
        edge: Profile,
    }

    fn world() -> World {
        World { pro: prof("Profile 1", "pro", "gabriel@work.com"), perso: prof("Default", "perso", "gab@perso.fr"), edge: prof("Default", "pro", "") }
    }

    fn live<'a>(link: u64, key: &'a str, name: &'a str, p: Option<&'a Profile>) -> Live<'a> {
        Live { link, key, name, profile: p }
    }

    #[test]
    fn the_first_seen_is_the_default_and_stays() {
        let w = world();
        let mut b = Book::default();
        assert!(b.seen("chrome", &w.perso));
        assert!(b.seen("chrome", &w.pro));
        assert_eq!(b.default, Some(Key { browser: "chrome".into(), dir: "Default".into() }));
        // hellos again, in any order: nothing changes
        assert!(!b.seen("chrome", &w.pro));
        assert!(!b.seen("chrome", &w.perso));
        // a renamed profile: its new name, same key
        assert!(b.seen("chrome", &prof("Profile 1", "work", "gabriel@work.com")));
        assert_eq!(b.known.len(), 2);
        assert_eq!(Book::from_json(&b.to_json()), b);
    }

    #[test]
    fn his_default_and_forget() {
        let w = world();
        let mut b = Book::default();
        b.seen("chrome", &w.perso);
        b.seen("chrome", &w.pro);
        let pro = Key { browser: "chrome".into(), dir: "Profile 1".into() };
        b.set_default(&pro).unwrap();
        assert_eq!(b.default.as_ref(), Some(&pro));
        assert!(b.set_default(&Key { browser: "edge".into(), dir: "Default".into() }).is_err());
        assert!(b.forget(&pro));
        assert_eq!(b.default.as_ref().map(|d| d.dir.as_str()), Some("Default"));
        assert!(!b.forget(&pro));
    }

    #[test]
    fn no_profile_goes_to_the_default_whatever_hello_came_last() {
        let w = world();
        let mut b = Book::default();
        b.seen("chrome", &w.pro);
        b.seen("chrome", &w.perso);
        // perso said hello last: first in the list, still pro
        let l = [live(2, "chrome", "Chrome", Some(&w.perso)), live(1, "chrome", "Chrome", Some(&w.pro))];
        assert_eq!(pick(&l, &b, None, None, None), Ok(1));
        // even when the agent's tabs are in perso: no profile asked is the default
        assert_eq!(pick(&l, &b, None, None, Some(2)), Ok(1));
        // browser chrome narrows nothing here
        assert_eq!(pick(&l, &b, Some("chrome"), None, None), Ok(1));
    }

    #[test]
    fn a_profile_by_name_email_or_dir_any_case() {
        let w = world();
        let mut b = Book::default();
        b.seen("chrome", &w.pro);
        b.seen("chrome", &w.perso);
        let l = [live(1, "chrome", "Chrome", Some(&w.pro)), live(2, "chrome", "Chrome", Some(&w.perso))];
        for q in ["perso", "PERSO", "gab@perso.fr", "default"] {
            assert_eq!(pick(&l, &b, None, Some(q), None), Ok(2), "{}", q);
        }
        for q in ["pro", "Gabriel@Work.com", "Profile 1"] {
            assert_eq!(pick(&l, &b, None, Some(q), None), Ok(1), "{}", q);
        }
    }

    #[test]
    fn unknown_is_refused_with_the_list_never_the_default() {
        let w = world();
        let mut b = Book::default();
        b.seen("chrome", &w.pro);
        b.seen("chrome", &w.perso);
        let l = [live(1, "chrome", "Chrome", Some(&w.pro))];
        let e = pick(&l, &b, None, Some("famille"), None).unwrap_err();
        assert_eq!(e.code, "no_browser");
        assert!(e.message.contains("Chrome · pro"), "{}", e.message);
        // known but closed: says to open it
        let e = pick(&l, &b, None, Some("perso"), None).unwrap_err();
        assert_eq!(e.code, "no_browser");
        assert!(e.message.contains("isn't open") && e.message.contains("Chrome"), "{}", e.message);
        let e = pick(&l, &b, Some("edge"), None, None).unwrap_err();
        assert_eq!(e.code, "no_browser");
    }

    #[test]
    fn two_matches_are_ambiguous_and_browser_settles_it() {
        let w = world();
        let mut b = Book::default();
        b.seen("chrome", &w.pro);
        b.seen("edge", &w.edge);
        let l = [live(1, "chrome", "Chrome", Some(&w.pro)), live(3, "edge", "Edge", Some(&w.edge))];
        let e = pick(&l, &b, None, Some("pro"), None).unwrap_err();
        assert_eq!(e.code, "ambiguous");
        assert_eq!(e.candidates, vec!["Chrome · pro".to_string(), "Edge · pro".to_string()]);
        assert_eq!(pick(&l, &b, Some("edge"), Some("pro"), None), Ok(3));
    }

    #[test]
    fn default_closed_one_left_or_the_agents_own_else_ask() {
        let w = world();
        let mut b = Book::default();
        b.seen("chrome", &w.perso);
        b.seen("chrome", &w.pro);
        b.seen("edge", &w.edge);
        // the default (perso) is closed; one other open: that one
        let one = [live(1, "chrome", "Chrome", Some(&w.pro))];
        assert_eq!(pick(&one, &b, None, None, None), Ok(1));
        // two others: the agent's own, else ambiguous with both
        let two = [live(1, "chrome", "Chrome", Some(&w.pro)), live(3, "edge", "Edge", Some(&w.edge))];
        assert_eq!(pick(&two, &b, None, None, Some(3)), Ok(3));
        let e = pick(&two, &b, None, None, None).unwrap_err();
        assert_eq!(e.code, "ambiguous");
        assert_eq!(e.candidates.len(), 2);
        // an extension that says no profile (older): alone, it works as before
        let old = [live(4, "chrome", "Chrome", None)];
        assert_eq!(pick(&old, &Book::default(), None, None, None), Ok(4));
        assert_eq!(pick(&old, &Book::default(), None, Some("pro"), None).unwrap_err().code, "no_browser");
    }

    #[test]
    fn agents_never_see_the_email() {
        let v = for_agents("chrome", &world().pro);
        assert_eq!(v, json!({"browser": "chrome", "dir": "Profile 1", "name": "pro"}));
        assert!(!v.to_string().contains('@'));
    }
}
