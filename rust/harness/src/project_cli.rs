//! `bise project add|list|remove|rename|move` (bise desktop, S1): the
//! projects registry from the terminal. Only `bise_home::projects` reads
//! and writes it; this module parses the words and prints. It never
//! starts or talks to a hub.
use bise_home::projects::{self, Added, Row};
use bise_home::style::Style;
use bise_home::Home;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const USAGE: &str = "bise project: the projects bise works in (the window's sidebar)

  bise project list [--json]           bise's own workspace, then each project
  bise project add [<folder>] [--name <name>]   add a folder (default: here)
  bise project remove <name|folder>    take it out of the list (its hub stays)
  bise project rename <name|folder> <new name>
  bise project move <name|folder> <position>    1 = first after bise";

/// A key the user typed: a folder that exists, canonical (the registry
/// keeps canonical paths), else the words as given (a name).
fn key(arg: &str) -> String {
    let p = Path::new(arg);
    if (arg.contains('/') || arg == "." || arg == "..") && p.exists() {
        return projects::canonical(p).to_string_lossy().to_string();
    }
    arg.to_string()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// One row of `list --json`.
pub fn row_json(r: &Row) -> Value {
    json!({
        "name": r.name, "path": r.path.to_string_lossy(), "id": r.id, "home": r.home,
        "added_ms": r.added_ms, "missing": !r.path.is_dir(),
    })
}

/// `list`'s text: one line per row, bise first.
pub fn list_text(rows: &[Row], st: &Style) -> String {
    let w = rows.iter().map(|r| r.name.chars().count()).max().unwrap_or(4);
    rows.iter()
        .map(|r| {
            let tail = if r.home {
                st.dim(" (bise's own)")
            } else if !r.path.is_dir() {
                st.dim(" (missing)")
            } else {
                String::new()
            };
            format!("{:w$}  {}{}\n", r.name, r.path.display(), tail)
        })
        .collect()
}

/// The value after `flag`, and the words that are not flags or values.
fn split(args: &[String], flag: &str) -> (Option<String>, Vec<String>) {
    let mut val = None;
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == flag {
            val = it.next().cloned();
        } else if !a.starts_with("--") {
            rest.push(a.clone());
        }
    }
    (val, rest)
}

pub fn main(args: &[String]) -> i32 {
    let home = Home::from_env();
    let home_ws = switchboard::paths::home_workspace();
    let cmd = args.first().map(String::as_str).unwrap_or("list");
    if args.iter().any(|a| a == "-h" || a == "--help") || cmd == "help" {
        println!("{USAGE}");
        return 0;
    }
    let rest = args.get(1..).unwrap_or(&[]);
    match run(cmd, rest, &home, &home_ws) {
        Ok(out) => {
            print!("{out}");
            0
        }
        Err(e) => {
            eprintln!("{}", Style::stderr().fail(&format!("bise project: {e}")));
            if e.starts_with("usage") {
                eprintln!("{USAGE}");
                2
            } else {
                1
            }
        }
    }
}

/// One command, its output or its error (`usage...` = wrong words).
pub fn run(cmd: &str, rest: &[String], home: &Home, home_ws: &Path) -> Result<String, String> {
    let usage = || Err(format!("usage: bise project {cmd} ..."));
    match cmd {
        "list" | "ls" => {
            let rows = projects::list(home, home_ws);
            if rest.iter().any(|a| a == "--json") {
                let v: Vec<Value> = rows.iter().map(row_json).collect();
                Ok(serde_json::to_string_pretty(&v).unwrap_or_default() + "\n")
            } else {
                Ok(list_text(&rows, &Style::stdout()))
            }
        }
        "add" => {
            let (name, words) = split(rest, "--name");
            if words.len() > 1 {
                return usage();
            }
            let dir = match words.first() {
                Some(d) => PathBuf::from(d),
                None => std::env::current_dir().map_err(|e| e.to_string())?,
            };
            match projects::add_path(home, home_ws, &dir, name.as_deref(), now_ms())? {
                Added::New(p) => Ok(format!("added {} ({})\n", p.name, p.path.display())),
                Added::Already(p) => Ok(format!("{} is already in the list as {}\n", p.path.display(), p.name)),
            }
        }
        "remove" | "rm" => {
            let [k] = rest else { return usage() };
            let p = projects::update(home, |l| projects::remove(l, &key(k)))?;
            Ok(format!("removed {} ({}); its hub's state stays\n", p.name, p.path.display()))
        }
        "rename" => {
            let [k, name] = rest else { return usage() };
            projects::update(home, |l| projects::rename(l, &key(k), name))?;
            Ok(format!("renamed to {}\n", name.trim()))
        }
        "move" | "mv" => {
            let [k, pos] = rest else { return usage() };
            let n: usize = pos.parse().ok().filter(|n| *n >= 1).ok_or_else(|| format!("{pos}: a position is 1, 2, ..."))?;
            projects::update(home, |l| projects::move_to(l, &key(k), n - 1))?;
            Ok(format!("moved to {n}\n"))
        }
        _ => Err(format!("usage: no command {cmd}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn add_list_rename_move_remove() {
        let root = projects::canonical(&std::env::temp_dir()).join(format!("bise-project-cli-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["bise", "code/api", "code/web"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        let home = Home::at(root.join(".bise"));
        let ws = root.join("bise");
        let api = root.join("code/api").to_string_lossy().to_string();
        let web = root.join("code/web").to_string_lossy().to_string();
        let run = |cmd: &str, rest: &[&str]| run(cmd, &s(rest), &home, &ws);
        assert!(run("add", &[&api]).unwrap().starts_with("added api"));
        assert!(run("add", &[&api]).unwrap().contains("already in the list as api"));
        assert!(run("add", &[&web, "--name", "site"]).unwrap().starts_with("added site"));
        assert!(run("add", &[ws.to_str().unwrap()]).unwrap_err().contains("bise's own"));
        assert!(run("add", &["a", "b"]).unwrap_err().starts_with("usage"));
        run("move", &["site", "1"]).unwrap();
        run("rename", &[&api, "backend"]).unwrap();
        let v: Value = serde_json::from_str(&run("list", &["--json"]).unwrap()).unwrap();
        let names: Vec<&str> = v.as_array().unwrap().iter().map(|r| r["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["bise", "site", "backend"]);
        assert_eq!(v[0]["home"], true);
        assert_eq!(v[2]["id"], bise_home::hub_id(&root.join("code/api")));
        assert!(run("move", &["site", "0"]).is_err());
        assert!(run("remove", &["bise"]).unwrap_err().contains("no project bise"));
        run("remove", &[&web]).unwrap();
        std::fs::remove_dir_all(root.join("code/api")).unwrap();
        let text = run("list", &[]).unwrap();
        assert!(text.contains("(bise's own)") && text.contains("backend") && text.contains("(missing)"), "{text}");
        assert!(!text.contains("site"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
