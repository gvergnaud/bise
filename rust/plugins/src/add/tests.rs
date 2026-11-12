//! The add menu on temp dirs and local git repos (no network).

use super::*;
use std::fs;

const SECRET: &str = "sk_live_SECRET_42";

struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn tmp(tag: &str) -> (Tmp, Ctx) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let d = std::env::temp_dir().join(format!("bp-add-{tag}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("home")).unwrap();
    let d = d.canonicalize().unwrap();
    let ctx = Ctx { builtin: Some(d.join("builtin")), ..Ctx::under(&d) };
    (Tmp(d), ctx)
}

fn write(p: &Path, text: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn manifest(name: &str) -> String {
    format!("{{\"$schema\": \"{PLUGIN_SCHEMA}\", \"name\": \"{name}\", \"version\": \"0.1.0\"}}")
}

/// Every file's text under `dir` (to look for a secret).
fn all_text(dir: &Path) -> String {
    let mut out = String::new();
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.push_str(&all_text(&p));
        } else if let Ok(t) = fs::read_to_string(&p) {
            out.push_str(&t);
        }
    }
    out
}

fn staging_is_empty(ctx: &Ctx) -> bool {
    fs::read_dir(staging_dir(&ctx.user_root)).map(|mut r| r.next().is_none()).unwrap_or(true)
}

fn json_of(p: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
}

fn mcp(name: &str, command: Option<&str>, url: Option<&str>, secrets: &[(&str, &str)]) -> Request {
    Request::Mcp {
        name: name.into(),
        command: command.map(str::to_string),
        url: url.map(str::to_string),
        secrets: secrets.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
    }
}

#[test]
fn an_mcp_command_is_previewed_as_argv_and_its_secret_kept_out_of_mcp_json() {
    let (_t, ctx) = tmp("mcp");
    let req = mcp("pay", Some("npx -y 'pay flows-mcp' --region \"eu west\""), None, &[("PAY_TOKEN", SECRET)]);
    let s = stage(&req, &ctx).unwrap();
    assert_eq!(s.preview.name, "pay");
    assert_eq!(s.preview.runs.len(), 1);
    assert_eq!(s.preview.runs[0].transport, RunTransport::Stdio);
    assert_eq!(s.preview.runs[0].argv, ["npx", "-y", "pay flows-mcp", "--region", "eu west"]);
    assert_eq!(s.preview.secrets, ["PAY_TOKEN"]);
    assert!(!ctx.user_root.join("pay").exists(), "nothing installed before his click");
    // never a value in the preview, its Debug, the request's Debug, or a staged file
    let shown = format!("{:?} {:?} {}", s, req, serde_json::to_string(&s.preview).unwrap());
    assert!(!shown.contains(SECRET), "{shown}");
    assert!(!all_text(&staging_dir(&ctx.user_root)).contains(SECRET));
    let dir = install(s, &ctx).unwrap();
    assert_eq!(dir, ctx.user_root.join("pay"));
    let m = json_of(&dir.join("mcp.json"));
    assert_eq!(m["mcpServers"]["pay"]["env"]["PAY_TOKEN"], "${PAY_TOKEN}");
    assert!(!all_text(&ctx.user_root).contains(SECRET), "no secret in the plugin");
    assert_eq!(crate::secrets::read(&ctx.secrets_dir, &PluginId::plugin("pay")).unwrap()["PAY_TOKEN"], SECRET);
    assert!(import::is_bise_mcp(&description(&dir).unwrap()));
    assert!(staging_is_empty(&ctx), "the staged folder goes");
}

#[test]
fn an_mcp_url_is_previewed_as_its_host_and_a_header_secret_becomes_a_var() {
    let (_t, ctx) = tmp("url");
    let s = stage(&mcp("notes", None, Some("https://mcp.notes.test/k3y/mcp"), &[("X-Api-Key", SECRET)]), &ctx).unwrap();
    assert_eq!(s.preview.runs, [PluginRun { server: "notes".into(), transport: RunTransport::Http, argv: vec![], host: Some("mcp.notes.test".into()) }]);
    assert_eq!(s.preview.secrets, ["X_API_KEY"]);
    assert!(!serde_json::to_string(&s.preview).unwrap().contains("k3y"), "a URL path may hold a key");
    let dir = install(s, &ctx).unwrap();
    assert_eq!(json_of(&dir.join("mcp.json"))["mcpServers"]["notes"]["headers"]["X-Api-Key"], "${X_API_KEY}");
    assert_eq!(crate::secrets::read(&ctx.secrets_dir, &PluginId::plugin("notes")).unwrap()["X_API_KEY"], SECRET);
    let sse = stage(&mcp("old", None, Some("https://old.test/sse"), &[]), &ctx).unwrap();
    assert_eq!(sse.preview.runs[0].transport, RunTransport::Sse);
    discard(sse);
    assert!(staging_is_empty(&ctx));
}

#[test]
fn a_bad_mcp_add_is_refused_by_kind_and_leaves_nothing() {
    let (_t, ctx) = tmp("badmcp");
    assert_eq!(stage(&mcp("Pay!", Some("x"), None, &[]), &ctx).unwrap_err(), AddError::BadName { name: "Pay!".into() });
    assert!(matches!(stage(&mcp("p", None, None, &[]), &ctx), Err(AddError::BadSource { .. })));
    assert!(matches!(stage(&mcp("p", Some("x"), Some("https://x.test"), &[]), &ctx), Err(AddError::BadSource { .. })));
    assert!(matches!(stage(&mcp("p", Some("x 'open"), None, &[]), &ctx), Err(AddError::BadSource { .. })));
    assert!(matches!(stage(&mcp("p", Some("x"), None, &[("NOT-A-VAR", "v")]), &ctx), Err(AddError::BadSource { .. })));
    assert!(matches!(stage(&mcp("p", Some("./rel/x"), None, &[]), &ctx), Err(AddError::NoServer { .. })));
    assert!(matches!(stage(&mcp("p", None, Some("ftp://x.test"), &[]), &ctx), Err(AddError::NoServer { .. })));
    assert!(staging_is_empty(&ctx));
    assert!(!ctx.user_root.join("p").exists());
}

#[test]
fn a_name_he_has_refuses_and_bise_own_mcp_folder_is_replaced() {
    let (_t, ctx) = tmp("collide");
    write(&ctx.user_root.join("pay/plugin.json"), &manifest("pay"));
    assert_eq!(stage(&mcp("pay", Some("npx pay"), None, &[]), &ctx).unwrap_err(), AddError::Exists { name: "pay".into() });
    // his plugin named pay in another folder: still his name
    write(&ctx.user_root.join("billing/plugin.json"), &manifest("billing2"));
    assert_eq!(stage(&mcp("billing2", Some("npx b"), None, &[]), &ctx).unwrap_err(), AddError::Exists { name: "billing2".into() });
    assert!(staging_is_empty(&ctx));
    // an MCP server he added: added again, it replaces its own folder
    install(stage(&mcp("notes", Some("npx notes-v1"), None, &[]), &ctx).unwrap(), &ctx).unwrap();
    let again = stage(&mcp("notes", Some("npx notes-v2"), None, &[]), &ctx).unwrap();
    assert_eq!(again.preview.replaces.as_deref(), Some("notes"));
    let dir = install(again, &ctx).unwrap();
    assert_eq!(json_of(&dir.join("mcp.json"))["mcpServers"]["notes"]["args"], json!(["notes-v2"]));
    // a plugin folder add never replaces, even bise's own
    let src = ctx.home.join("src/notes");
    write(&src.join("plugin.json"), &manifest("notes"));
    assert_eq!(stage(&Request::Folder { path: src }, &ctx).unwrap_err(), AddError::Exists { name: "notes".into() });
}

#[test]
fn a_built_in_of_the_same_name_is_said() {
    let (_t, ctx) = tmp("shadow");
    write(&ctx.builtin.clone().unwrap().join("docs/plugin.json"), &manifest("docs"));
    let s = stage(&mcp("docs", Some("npx docs-mcp"), None, &[]), &ctx).unwrap();
    assert!(s.preview.notes.contains(&"it replaces bise's own docs part".to_string()), "{:?}", s.preview.notes);
}

#[test]
fn a_folder_plugin_is_copied_and_its_preview_shows_its_installed_paths() {
    let (_t, ctx) = tmp("folder");
    let src = ctx.home.join("code/hello");
    write(&src.join("plugin.json"), &manifest("hello"));
    write(&src.join("skills/greet/SKILL.md"), "---\nname: greet\ndescription: say hi\n---\nHi.\n");
    write(&src.join("bin/server"), "#!/bin/sh\n");
    write(
        &src.join("mcp.json"),
        r#"{"mcpServers": {"hi": {"command": "${PLUGIN_ROOT}/bin/server", "args": ["--data", "${PLUGIN_ROOT}/d"]}, "py": {"command": "uvx", "args": ["hi-mcp"]}}}"#,
    );
    write(&src.join(".git/HEAD"), "ref: refs/heads/main\n");
    let s = stage(&Request::Folder { path: src.clone() }, &ctx).unwrap();
    let dest = ctx.user_root.join("hello");
    let argvs: Vec<Vec<String>> = s.preview.runs.iter().map(|r| r.argv.clone()).collect();
    assert_eq!(
        argvs,
        [vec![format!("{}/bin/server", dest.display()), "--data".into(), format!("{}/d", dest.display())], vec!["uvx".to_string(), "hi-mcp".into()]]
    );
    assert_eq!(s.preview.skills, ["hello:greet"]);
    assert_eq!(s.preview.from, "~/code/hello");
    install(s, &ctx).unwrap();
    assert!(dest.join("skills/greet/SKILL.md").is_file());
    assert!(!dest.join(".git").exists(), ".git is left out");
    assert!(src.join("plugin.json").is_file(), "his folder stays");
}

#[test]
fn a_bad_manifest_is_refused_before_the_move() {
    let (_t, ctx) = tmp("badmanifest");
    let src = ctx.home.join("bad");
    write(&src.join("plugin.json"), r#"{"name": "bad", "version": "1", "hooks": "./h.json"}"#);
    match stage(&Request::Folder { path: src }, &ctx) {
        Err(AddError::Invalid { diagnostics }) => assert!(diagnostics.join(" ").contains("hooks"), "{diagnostics:?}"),
        other => panic!("{other:?}"),
    }
    let none = ctx.home.join("none");
    fs::create_dir_all(&none).unwrap();
    assert_eq!(stage(&Request::Folder { path: none }, &ctx).unwrap_err(), AddError::NoPlugin);
    assert!(matches!(stage(&Request::Folder { path: ctx.home.join("missing") }, &ctx), Err(AddError::BadSource { .. })));
    let reserved = ctx.home.join("self");
    write(&reserved.join("plugin.json"), &manifest("self"));
    assert!(matches!(stage(&Request::Folder { path: reserved }, &ctx), Err(AddError::Invalid { .. })));
    assert!(!ctx.user_root.join("bad").exists() && !ctx.user_root.join("self").exists());
    assert!(staging_is_empty(&ctx));
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/add").join(name)
}

#[test]
fn a_codex_plugin_becomes_its_expected_agent_plugin() {
    let (_t, ctx) = tmp("codex");
    let s = stage(&Request::Folder { path: fixture("codex-linear/in") }, &ctx).unwrap();
    assert_eq!(s.preview.runs, [PluginRun { server: "linear".into(), transport: RunTransport::Http, argv: vec![], host: Some("mcp.linear.app".into()) }]);
    assert_eq!(s.preview.skills, ["linear:triage"]);
    // defaultPrompt became dev.bise.prompts; the other interface keys are named
    assert_eq!(
        s.preview.dropped,
        ["plugin.json: apps", "plugin.json: interface.category", "plugin.json: interface.displayName", "plugin.json: interface.shortDescription", "linear: oauth_resource"]
    );
    let dir = install(s, &ctx).unwrap();
    assert_eq!(json_of(&dir.join("plugin.json")), json_of(&fixture("codex-linear/want/plugin.json")));
    assert_eq!(json_of(&dir.join("mcp.json")), json_of(&fixture("codex-linear/want/mcp.json")));
}

/// A Codex plugin `name` whose `interface.defaultPrompt` is `prompt`
/// (none: no key), staged and installed: its preview's dropped list and
/// the resolver's plugin.
fn codex_with(ctx: &Ctx, name: &str, prompt: Option<Value>) -> (Vec<String>, resolve::Plugin, Vec<resolve::Diagnostic>) {
    let src = ctx.home.join(format!("src-{name}"));
    let mut iface = json!({ "displayName": "X" });
    if let Some(p) = prompt {
        iface["defaultPrompt"] = p;
    }
    write(&src.join(".codex-plugin/plugin.json"), &json!({ "name": name, "description": "d", "interface": iface }).to_string());
    write(&src.join("skills/s/SKILL.md"), "---\nname: s\ndescription: a skill\n---\nbody\n");
    let s = stage(&Request::Folder { path: src }, ctx).unwrap();
    let dropped = s.preview.dropped.clone();
    install(s, ctx).unwrap();
    let state = StateV2 { enabled: vec![name.into()], ..StateV2::default() };
    let r = resolve::resolve(&Roots { builtin: None, user: Some(ctx.user_root.clone()), workspace: None, data: ctx.data.clone(), documents: ctx.documents.clone(), state });
    let p = r.plugins.into_iter().find(|p| p.name == name).unwrap();
    let mine = r.diagnostics.into_iter().filter(|d| d.plugin == name).collect();
    (dropped, p, mine)
}

#[test]
fn codex_default_prompts_become_bise_prompts_cut_by_the_resolver_rule() {
    let (_t, ctx) = tmp("prompts");
    // a string
    let (dropped, p, diags) = codex_with(&ctx, "one", Some(json!("Triage my\n  issues")));
    assert_eq!(p.prompts, ["Triage my issues"]);
    assert_eq!(p.state, State::Loaded, "{diags:?}");
    assert_eq!(dropped, ["plugin.json: interface.displayName"]);
    // a list of 5: the first 3, and the cut named
    let five = json!(["a", "b", "", "c", "d", 7]);
    let (dropped, p, _) = codex_with(&ctx, "five", Some(five));
    assert_eq!(p.prompts, ["a", "b", "c"]);
    assert_eq!(dropped, ["plugin.json: interface.defaultPrompt beyond 3 prompts of 120 characters", "plugin.json: interface.displayName"]);
    // a 300-character prompt: cut as the resolver cuts, never a load warning after
    let long = "word ".repeat(60);
    let (dropped, p, diags) = codex_with(&ctx, "long", Some(json!(long)));
    assert_eq!(p.prompts, resolve::cut_prompts(&[long.trim().to_string()]).0);
    assert!(p.prompts[0].chars().count() <= resolve::PROMPT_CHARS);
    assert!(dropped[0].starts_with("plugin.json: interface.defaultPrompt beyond"), "{dropped:?}");
    assert!(!diags.iter().any(|d| d.code == "plugin.prompts.cut"), "{diags:?}");
    // none, and an empty one: no extensions written
    for (name, prompt) in [("none", None), ("blank", Some(json!(["  "])))] {
        let (_, p, _) = codex_with(&ctx, name, prompt);
        assert!(p.prompts.is_empty());
        assert!(json_of(&ctx.user_root.join(name).join("plugin.json")).get("extensions").is_none(), "{name}");
    }
}

/// A stdio MCP server listing two tools (`pid_file`: its pid, to check
/// the process is gone after the try).
const TWO_TOOLS: &str = r#"
import json, os, sys
open(sys.argv[1], "w").write(str(os.getpid()))
for line in sys.stdin:
    m = json.loads(line)
    if "id" not in m:
        continue
    if m["method"] == "initialize":
        r = {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}}, "serverInfo": {"name": "two", "version": "1"}}
    elif m["method"] == "tools/list":
        r = {"tools": [{"name": "a", "description": "first", "inputSchema": {"type": "object"}},
                       {"name": "b", "description": "second", "inputSchema": {"type": "object"}}]}
    else:
        r = {}
    print(json.dumps({"jsonrpc": "2.0", "id": m["id"], "result": r}), flush=True)
"#;

/// The fake remote server (tests/fake_mcp_http.py), killed on drop.
struct FakeHttp(std::process::Child, u16);
impl Drop for FakeHttp {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn fake_http(dir: &Path, args: &[&str]) -> FakeHttp {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fake_mcp_http.py");
    let port_file = dir.join("port");
    let child = std::process::Command::new("python3").arg(&script).arg("--port-file").arg(&port_file).args(args).spawn().unwrap();
    // owned at once: its drop kills and waits on every path
    let mut fake = FakeHttp(child, 0);
    let t0 = std::time::Instant::now();
    while fake.1 == 0 {
        match fs::read_to_string(&port_file).ok().and_then(|s| s.trim().parse().ok()) {
            Some(p) => fake.1 = p,
            None => {
                assert!(t0.elapsed() < Duration::from_secs(20), "the fake server never started");
                std::thread::sleep(Duration::from_millis(30));
            }
        }
    }
    fake
}

#[test]
fn the_servers_are_tried_once_right_after_the_confirm_and_stopped() {
    let (_t, ctx) = tmp("try");
    let http = fake_http(&ctx.home, &["--mode", "streamable", "--require-header", "X-Key=right"]);
    let src = ctx.home.join("src");
    let pid_file = ctx.home.join("two.pid");
    write(&src.join("plugin.json"), &manifest("tried"));
    write(&src.join("two.py"), TWO_TOOLS);
    let mcp = json!({ "$schema": resolve::MCP_SCHEMA, "mcpServers": {
        "two": { "command": "python3", "args": ["-B", "${PLUGIN_ROOT}/two.py", pid_file.display().to_string()] },
        "dies": { "command": "sh", "args": ["-c", "echo no DATABASE_URL >&2; exit 3"], "startup_timeout_sec": 5 },
        "web": { "url": format!("http://127.0.0.1:{}/mcp", http.1), "startup_timeout_sec": 5 }
    }});
    write(&src.join("mcp.json"), &mcp.to_string());
    let s = stage(&Request::Folder { path: src }, &ctx).unwrap();
    // staged and previewed: nothing ran
    assert!(!pid_file.exists() && !ctx.status_dir.exists(), "nothing runs before his click");
    let dir = install(s, &ctx).unwrap();
    assert!(!pid_file.exists(), "the install runs nothing either");

    let mut got = try_servers(&dir, &ctx);
    got.sort_by(|a, b| a.0.cmp(&b.0));
    let ids: Vec<&str> = got.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["dies", "two", "web"]);
    let read = |id: &str| crate::status::read(&ctx.status_dir, "tried", id).unwrap();
    let two = read("two");
    assert_eq!(two.tools, Ok(2));
    assert_eq!(two.listed, vec![("a".to_string(), "first".to_string()), ("b".to_string(), "second".to_string())]);
    let dies = read("dies").tools.unwrap_err();
    assert!(dies.contains("no DATABASE_URL"), "{dies}");
    let web = read("web");
    assert!(web.login, "a 401 is a login, not an error: {web:?}");
    // the server's process is gone after the try
    let pid: i32 = fs::read_to_string(&pid_file).unwrap().trim().parse().unwrap();
    assert!(unsafe { libc::kill(pid, 0) } != 0, "the tried server still runs");
    // the plugin stays installed, and nothing is left in staging
    assert!(dir.join("plugin.json").is_file());
    assert!(staging_is_empty(&ctx));
}

#[test]
fn a_claude_plugin_becomes_its_expected_agent_plugin() {
    let (_t, ctx) = tmp("claude");
    let s = stage(&Request::Folder { path: fixture("claude-notes/in") }, &ctx).unwrap();
    let dest = ctx.user_root.join("notes");
    let argvs: Vec<Vec<String>> = s.preview.runs.iter().map(|r| r.argv.clone()).collect();
    assert_eq!(
        argvs,
        [vec![format!("{}/bin/notes-mcp", dest.display()), "--dir".into(), format!("{}/data", dest.display())], vec!["npx".into(), "-y".into(), "notes-search-mcp".into()]]
    );
    assert_eq!(s.preview.dropped, ["plugin.json: commands", "search: alwaysAllow"]);
    assert!(s.preview.notes.iter().any(|n| n.contains("Claude Code plugin")), "{:?}", s.preview.notes);
    let dir = install(s, &ctx).unwrap();
    assert_eq!(json_of(&dir.join("plugin.json")), json_of(&fixture("claude-notes/want/plugin.json")));
    assert_eq!(json_of(&dir.join("mcp.json")), json_of(&fixture("claude-notes/want/mcp.json")));
}

/// The folder `dir` made a local git repo, its files committed.
pub(crate) fn git_repo(dir: &Path) {
    let git = |args: &[&str]| {
        let st = std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(st.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "init"]);
}

#[test]
fn a_git_plugin_is_cloned_converted_and_installed_from_its_folder() {
    let (_t, ctx) = tmp("git");
    let repo = ctx.home.join("repo");
    fs::create_dir_all(repo.join("plugins")).unwrap();
    copy_tree(&fixture("codex-linear/in"), &repo.join("plugins/linear")).unwrap();
    git_repo(&repo);
    let req = Request::Git { url: repo.display().to_string(), subdir: Some("./plugins/linear".into()) };
    let s = stage(&req, &ctx).unwrap();
    assert_eq!(s.preview.from, format!("git {} (plugins/linear)", url_shown(&repo.display().to_string())));
    assert_eq!(s.preview.runs[0].host.as_deref(), Some("mcp.linear.app"));
    let dir = install(s, &ctx).unwrap();
    assert!(dir.join("skills/triage/SKILL.md").is_file());
    assert!(!dir.join(".git").exists());
    // no such folder, a folder outside the repo, a repo that isn't there
    for sub in ["plugins/none", "../repo", "/etc"] {
        let r = Request::Git { url: repo.display().to_string(), subdir: Some(sub.into()) };
        assert!(matches!(stage(&r, &ctx), Err(AddError::BadSource { .. })), "{sub}");
    }
    let r = Request::Git { url: ctx.home.join("nothing").display().to_string(), subdir: None };
    assert!(matches!(stage(&r, &ctx), Err(AddError::Git { .. })));
    assert!(staging_is_empty(&ctx));
}

#[test]
fn imports_are_found_on_disk_and_staged_from_each_agent() {
    let (_t, ctx) = tmp("import");
    write(
        &ctx.home.join(".claude.json"),
        &json!({"numStartups": 3, "mcpServers": {"gh": {"command": "npx", "args": ["-y", "@mcp/github"], "env": {"GITHUB_TOKEN": SECRET}}, "lin": {"type": "http", "url": "https://mcp.linear.app/mcp"}}}).to_string(),
    );
    write(&ctx.home.join(".codex/config.toml"), "model = \"o3\"\n\n[mcp_servers.docs]\ncommand = \"uvx\"\nargs = [\"docs-mcp\"]\ntool_timeout_sec = 60\n\n[mcp_servers.off]\ncommand = \"x\"\nenabled = false\n");
    write(&ctx.home.join(".cursor/mcp.json"), r#"{"mcpServers": {"pg": {"command": "/opt/pg-mcp"}}}"#);
    let f = found(&ctx);
    let got: Vec<(ImportFrom, Vec<String>, Option<String>)> = f.iter().map(|f| (f.from, f.servers.clone(), f.added.clone())).collect();
    assert_eq!(
        got,
        [
            (ImportFrom::Claude, vec!["gh".to_string(), "lin".into()], None),
            (ImportFrom::Codex, vec!["docs".to_string()], None),
            (ImportFrom::Cursor, vec!["pg".to_string()], None),
        ]
    );
    let s = stage(&Request::Import { from: ImportFrom::Codex, name: None }, &ctx).unwrap();
    assert_eq!(s.preview.name, "from-codex");
    assert_eq!(s.preview.from, "Codex (~/.codex/config.toml)");
    assert_eq!(s.preview.runs[0].argv, ["uvx", "docs-mcp"]);
    assert_eq!(s.preview.dropped, ["off (disabled there)"]);
    install(s, &ctx).unwrap();
    let s = stage(&Request::Import { from: ImportFrom::Cursor, name: None }, &ctx).unwrap();
    assert_eq!(s.preview.runs[0].argv, ["sh", "-c", "exec \"$0\" \"$@\"", "/opt/pg-mcp"]);
    discard(s);
    // Claude Code's token stays in its env, as import-mcp does, and never in the preview
    let s = stage(&Request::Import { from: ImportFrom::Claude, name: None }, &ctx).unwrap();
    assert!(!serde_json::to_string(&s.preview).unwrap().contains(SECRET));
    discard(s);
    assert_eq!(found(&ctx)[1].added.as_deref(), Some("from-codex"), "imported: said");
    // imported again: it replaces its own
    assert_eq!(stage(&Request::Import { from: ImportFrom::Codex, name: None }, &ctx).unwrap().preview.replaces.as_deref(), Some("from-codex"));
    // none there, or not parsing
    let (_t2, empty) = tmp("import-none");
    assert!(found(&empty).is_empty());
    write(&empty.home.join(".codex/config.toml"), "[[[");
    assert!(matches!(stage(&Request::Import { from: ImportFrom::Codex, name: None }, &empty), Err(AddError::BadSource { .. })));
}

#[test]
fn a_failed_secrets_write_installs_nothing() {
    let (_t, mut ctx) = tmp("secretsfail");
    let s = stage(&mcp("pay", Some("npx pay"), None, &[("PAY_TOKEN", SECRET)]), &ctx).unwrap();
    // the secrets folder can't be made: a file is in its way
    write(&ctx.secrets_dir.clone(), "x");
    ctx.secrets_dir = ctx.secrets_dir.join("mcp-env");
    assert!(matches!(install(s, &ctx), Err(AddError::Secrets { .. })));
    assert!(!ctx.user_root.join("pay").exists());
    assert!(staging_is_empty(&ctx));
}

#[test]
fn old_staging_folders_are_swept() {
    let (_t, ctx) = tmp("sweep");
    let st = staging_dir(&ctx.user_root);
    fs::create_dir_all(st.join("old/x")).unwrap();
    fs::create_dir_all(st.join("new/x")).unwrap();
    let day_ago = std::time::SystemTime::now() - Duration::from_secs(25 * 3600);
    fs::File::open(st.join("old")).unwrap().set_modified(day_ago).unwrap();
    sweep(&st, STAGING_TTL);
    assert!(!st.join("old").exists());
    assert!(st.join("new").exists());
}

#[test]
fn words_urls_and_the_agent_brief() {
    assert_eq!(split_words(r#"a 'b c' "d \"e\"" f\ g"#).unwrap(), ["a", "b c", "d \"e\"", "f g"]);
    assert_eq!(split_words("  "), None);
    assert_eq!(split_words("a 'b"), None);
    assert_eq!(git_url("openai/plugins").unwrap(), "https://github.com/openai/plugins.git");
    assert_eq!(git_url("github.com/openai/plugins").unwrap(), "https://github.com/openai/plugins.git");
    assert_eq!(git_url("git@github.com:a/b.git").unwrap(), "git@github.com:a/b.git");
    for bad in ["-c x", "ext::sh -c id", "a b", "", "a/b/c", "../x"] {
        assert!(git_url(bad).is_err(), "{bad}");
    }
    assert_eq!(url_shown("https://u:pw@github.com/a/b.git?x=1"), "github.com/a/b");
    let b = agent_brief("  reads our Grafana dashboards ");
    assert!(b.contains("plugin-creator") && b.contains("reads our Grafana dashboards.") && b.contains("/plugins add"), "{b}");
}

#[test]
fn the_core_holds_previews_by_token_until_his_answer() {
    let (_t, ctx) = tmp("held");
    let mut h = Held::default();
    let (tok, p) = h.hold(stage(&mcp("a", Some("npx a"), None, &[]), &ctx).unwrap());
    assert_eq!(p.name, "a");
    let (tok_b, _) = h.hold(stage(&mcp("b", Some("npx b"), None, &[]), &ctx).unwrap());
    assert_eq!(h.confirm("nope", &[], &ctx).unwrap_err(), AddError::UnknownToken);
    assert!(h.confirm(&tok, &[], &ctx).unwrap().join("mcp.json").is_file());
    assert_eq!(h.confirm(&tok, &[], &ctx).unwrap_err(), AddError::UnknownToken, "used once");
    h.cancel(&tok_b).unwrap();
    assert_eq!(h.cancel(&tok_b).unwrap_err(), AddError::UnknownToken);
    assert!(!ctx.user_root.join("b").exists());
    h.hold(stage(&mcp("c", Some("npx c"), None, &[]), &ctx).unwrap());
    drop(h);
    assert!(staging_is_empty(&ctx), "a closed window leaves no staged folder");
}

/// architect m_39668: a preview he never answered (a window closed
/// without a cancel) goes after HELD_TTL, its staged folder with it.
#[test]
fn a_held_preview_goes_after_its_ttl() {
    let (_t, ctx) = tmp("held-ttl");
    let mut h = Held::default();
    let (tok, _) = h.hold(stage(&mcp("a", Some("npx a"), None, &[]), &ctx).unwrap());
    let now = std::time::Instant::now();
    h.sweep(now);
    assert!(!staging_is_empty(&ctx), "still waiting for him");
    h.sweep(now + HELD_TTL);
    assert_eq!(h.cancel(&tok).unwrap_err(), AddError::UnknownToken);
    assert!(staging_is_empty(&ctx), "its staged folder went with it");
}

#[test]
fn the_wire_add_carries_secret_names_only_and_writes_no_value() {
    use bise_proto::app_cmd::PluginAddKind as K;
    let (_t, ctx) = tmp("wire");
    let add = K::Mcp { name: "pay".into(), command: Some("npx pay-mcp".into()), url: None, secrets: vec!["PAY_TOKEN".into()] };
    let req = Request::from_wire(&add).unwrap();
    let s = stage(&req, &ctx).unwrap();
    assert_eq!(s.preview.secrets, ["PAY_TOKEN"]);
    let dir = install(s, &ctx).unwrap();
    assert_eq!(json_of(&dir.join("mcp.json"))["mcpServers"]["pay"]["env"]["PAY_TOKEN"], "${PAY_TOKEN}");
    assert!(!crate::secrets::path(&ctx.secrets_dir, &PluginId::plugin("pay")).exists(), "no empty value written");
    assert_eq!(Request::from_wire(&K::Git { url: "a/b".into(), subdir: Some("p".into()) }), Some(Request::Git { url: "a/b".into(), subdir: Some("p".into()) }));
    assert_eq!(Request::from_wire(&K::Agent { words: "x".into() }), None);
    assert_eq!(Request::from_wire(&K::Marketplace { url: "a/b".into() }), None);
}

#[test]
fn the_confirm_carries_the_values_of_the_preview_secrets_only() {
    use bise_proto::app_cmd::{PluginAddKind as K, Secret};
    let (_t, ctx) = tmp("confirm-secrets");
    let mut h = Held::default();
    let add = K::Mcp { name: "pay".into(), command: Some("npx pay-mcp".into()), url: None, secrets: vec!["PAY_TOKEN".into(), "PAY_ORG".into()] };
    let (tok, _) = h.hold(stage(&Request::from_wire(&add).unwrap(), &ctx).unwrap());
    let v = |n: &str, s: &str| SecretValue { name: n.into(), value: Secret::new(s) };
    // a name the preview doesn't list: refused, nothing written, still held
    assert_eq!(h.confirm(&tok, &[v("PAY_TOKEN", SECRET), v("OTHER", "x")], &ctx).unwrap_err(), AddError::UnknownSecret { name: "OTHER".into() });
    assert!(!ctx.secrets_dir.exists() && !ctx.user_root.join("pay").exists());
    // PAY_ORG left out: it stays ${PAY_ORG}
    let dir = h.confirm(&tok, &[v("PAY_TOKEN", SECRET)], &ctx).unwrap();
    let kept = crate::secrets::read(&ctx.secrets_dir, &PluginId::plugin("pay")).unwrap();
    assert_eq!(kept.into_iter().collect::<Vec<_>>(), [("PAY_TOKEN".to_string(), SECRET.to_string())]);
    let m = json_of(&dir.join("mcp.json"));
    assert_eq!(m["mcpServers"]["pay"]["env"], json!({"PAY_ORG": "${PAY_ORG}", "PAY_TOKEN": "${PAY_TOKEN}"}));
    // the only file holding the value is the secrets store's
    assert!(!all_text(&ctx.user_root).contains(SECRET));
    assert!(!format!("{:?}", v("PAY_TOKEN", SECRET)).contains(SECRET), "Debug never shows it");
}
