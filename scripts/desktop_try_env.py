#!/usr/bin/env python3
"""desktop_try_env: the pure where and what of scripts/desktop_try.py, split out under its line cap
(main m_20872): the try's homes and guard, its environment, his repos, the clone names, the try's
config.toml. desktop_try imports them; tested in tests/desktop_try_test.py."""
import json
import os
import re
import sys


REPO = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
sys.path.insert(0, os.path.join(REPO, "tests"))
from bise_env import NOT_INHERITED, OWN_PATHS  # noqa: E402  (the one list, never copied)


def his_bise():
    """His own ~/.bise from the user database, never from HOME: a run with HOME on a stand-in
    (a gate world, the app's swap from a stand-in) must never take that home for his."""
    import pwd
    return os.path.join(pwd.getpwuid(os.getuid()).pw_dir, ".bise")


# his gate, whatever HOME says (a stand-in's HOME would put it inside the stand-in)
GATE = os.path.realpath(os.path.join(his_bise(), "gate"))


class TryError(Exception):
    pass


# ---------------------------------------------------------------- where (pure)

def homes(env=None):
    """(his real ~/.bise, the try home). Tests point both under ~/.bise/gate."""
    env = os.environ if env is None else env
    real = os.path.realpath(os.path.expanduser(env.get("BISE_TRY_REAL") or "~/.bise"))
    tryh = os.path.realpath(os.path.expanduser(env.get("BISE_TRY_HOME") or os.path.join(real, "try", "desktop")))
    return real, tryh


def under(path, root):
    p, r = os.path.realpath(path), os.path.realpath(root)
    return p == r or p.startswith(r.rstrip("/") + "/")


SELF_CONTAINED = ("bise", "VERSION", "repl-live", "sb-core")


def self_contained(app):
    """The bundle runs with no variable: its runtime is in Contents/Resources/bin (amb-core m_18365,
    update.rs REQUIRED). Returns what is missing."""
    return [n for n in SELF_CONTAINED if not os.path.exists(os.path.join(app, "Contents", "Resources", "bin", n))]


def live_build(app):
    """The build a bundle carries: its runtime's VERSION (Contents/Resources/bin/VERSION, id= and
    commit=), never the try checkout's HEAD (ambient-lead m_21726). {} when it has none."""
    try:
        text = open(os.path.join(app, "Contents", "Resources", "bin", "VERSION")).read()
    except OSError:
        return {}
    return dict(l.split("=", 1) for l in text.splitlines() if "=" in l)


def keep_old_root(saved):
    """The previous bundle's root, carried by a new stamp until the next swap removes it. Pure."""
    return {"old_root": saved["old_root"]} if (saved or {}).get("old_root") else {}


def holds_bundle(cmd, app):
    """Whether a process (its `ps` command line) holds the bundle `app`: it runs from it, and it is
    not the browsers' native host, `<packed bise> computer-use chrome-host chrome-extension://…`,
    nor the computer-use broker, `<packed bise> computer-use broker [args]`.
    The host is exempt: a stateless relay (rust/computer-use/src/host.rs: frames to the broker's
    socket; the broker is its own process, outside the bundle) that the browser starts through
    ~/.bise/bin/bise-chrome-host and starts again within 2 s when killed, so no swap could wait it
    out. A rename leaves it on its old inode, relaying frames as they come to the same broker, until
    the browser starts it again, from the new bundle (same path): what the app's own updater does
    to every user (apps/desktop/src/main/update/swap.ts renames with no process check). Pure."""
    contents = app.rstrip("/") + "/Contents/"
    if not cmd.startswith(contents):
        return False
    bise = os.path.join(contents, "Resources", "bin", "bise")
    host = bise + " computer-use chrome-host chrome-extension://"
    # the computer-use broker too (keep-current m_61362, m_61383): the host's child, from the
    # bundle; killed, the host starts it again within 1 s, so it refused every swap. Renamed under
    # it, it keeps its old inode until Chrome restarts the host, which starts the new bundle's
    broker = bise + " computer-use broker"
    return not (cmd.startswith(host) or cmd == broker or cmd.startswith(broker + " "))


def not_swapped(staged, live, procs, most=6):
    """The loud line of --real when its staged build did not go live (ambient-lead m_21726): the
    OLD build reopens, and what held the bundle, each process's pid and command. Pure."""
    held = "; ".join("pid %d %s" % (p, c) for p, c in procs[:most]) + ("; +%d more" % (len(procs) - most) if len(procs) > most else "")
    return ("NOT UPDATED: reopening the OLD build %s; the staged build %s did not go live, %d processes still run from"
            " the bundle: %s. quit them (bise stop in each of their workspaces), then --real again"
            % (live or "unknown", staged or "unknown", len(procs), held or "none seen"))


def guard(real, tryh, paths=()):
    """The try home is under his ~/.bise/try (or a test's ~/.bise/gate), never his home itself;
    every path the try writes is under it."""
    if not (under(tryh, os.path.join(real, "try")) or under(tryh, GATE)) or os.path.realpath(tryh) == os.path.realpath(real):
        raise TryError("refused: the try home %s is not under %s/try" % (tryh, real))
    for p in paths:
        if not under(p, tryh):
            raise TryError("refused: %s is outside the try home %s" % (p, tryh))


def try_env(base, real, tryh, repo=REPO):
    """The environment of everything the try runs: his, minus every bise path override and
    internal variable (an agent's shell carries them; a value pointing at his ~/.bise would be
    kept, architect m_9176 (2)), plus the try home and its own home workspace."""
    drop = set(OWN_PATHS) | set(NOT_INHERITED) | {"BISE_HOME"}
    env = {k: v for k, v in base.items() if k not in drop and not k.startswith("SB_") and k != "BISE_ROLE"}
    env.update(BISE_HOME=tryh, BISE_HOME_WORKSPACE=os.path.join(tryh, "bise"), BISE_APP_ROOT=repo)
    if "PATH" in env:
        env["PATH"] = clean_path(env["PATH"], real, homes=(os.path.expanduser("~/.bise"),))
    return env


def clean_path(path, real, homes=()):
    """PATH without any bise state's hubs/ and run/ folders: an agent's shell has
    <his ~/.bise>/hubs/<hub>/bin first (its sb talks to HIS hub); the try's processes must never
    find that one. `homes` adds other bise homes to strip (the default ~/.bise when `real` is a
    stand-in, as in the try-proof)."""
    roots = [real, *homes]
    bad = [os.path.join(r, d) for r in roots for d in ("hubs", "run")]
    keep = [p for p in path.split(":") if p and not any(under(p, b) for b in bad)]
    return ":".join(keep)


def his_env(base):
    """His own environment for the build (his build dir comes from it): no BISE_HOME, no agent's or
    hub's variables."""
    drop = set(OWN_PATHS) | set(NOT_INHERITED) | {"BISE_HOME"}
    return {k: v for k, v in base.items() if k not in drop and not k.startswith("SB_") and k != "BISE_ROLE"}


# ---------------------------------------------------------------- what (pure)

def his_repos(projects_text):
    """The paths of his projects.json ({"v":1,"projects":[{path,name,order}]}), by order."""
    try:
        d = json.loads(projects_text)
    except ValueError:
        return []
    rows = sorted((p for p in d.get("projects", []) if isinstance(p, dict) and p.get("path")), key=lambda p: p.get("order", 0))
    return [p["path"] for p in rows]


def clone_names(paths):
    """A folder name per repo under <try>/repos: its basename, '-2', '-3' when taken."""
    out, taken = [], set()
    for p in paths:
        base = re.sub(r"[^A-Za-z0-9._-]", "-", os.path.basename(os.path.normpath(p))) or "repo"
        name, i = base, 2
        while name in taken:
            name, i = "%s-%d" % (base, i), i + 1
        taken.add(name)
        out.append(name)
    return out


def config_for_try(his_config, his_auth, real, bise):
    """The try's config.toml: his, minus the [pages] table (its mirror would publish to his
    site), plus a key_command per entry of his auth.json that reads it there at each call:
    an API key with jq (read only), a ChatGPT sign-in through HIS bise home (his grant, his lock).
    Never a key or a token in the text."""
    lines, skip = [], False
    for line in his_config.splitlines():
        head = re.match(r"\s*\[([^\]]+)\]\s*$", line)
        if head:
            skip = head.group(1).strip() == "pages" or head.group(1).strip().startswith("pages.")
        if not skip:
            lines.append(line)
    auth_file = os.path.join(real, "auth.json")
    for pid, entry in sorted((his_auth or {}).items()):
        if not isinstance(entry, dict) or not re.fullmatch(r"[A-Za-z0-9_-]+", pid):
            continue
        if entry.get("type") == "api" or "key" in entry:
            cmd = "jq -r .%s.key '%s'" % (pid, auth_file)
        elif pid == "chatgpt":
            cmd = "BISE_HOME='%s' '%s' auth token chatgpt" % (real, bise)
        else:
            continue
        if '"' in cmd or "\\" in cmd:
            raise TryError("a path with a quote or a backslash can't go in a key_command: %s" % cmd)
        table = "[providers.%s]" % pid
        idx = next((i for i, l in enumerate(lines) if l.strip() == table), None)
        kc = 'key_command = "%s"' % cmd
        if idx is None:
            lines += ["", table, kc]
        else:
            j = idx + 1
            while j < len(lines) and not lines[j].strip().startswith("["):
                if lines[j].strip().startswith("key_command"):
                    lines.pop(j)
                    continue
                j += 1
            lines.insert(idx + 1, kc)
    return "\n".join(lines).rstrip() + "\n"
