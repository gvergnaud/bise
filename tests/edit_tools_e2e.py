#!/usr/bin/env python3
"""approvals-edit (design docs/approvals-design.md §2.2): Vibe's `edit`
and `write_file`, one edit toolset per request, on a real repl-live
against the fake provider.

A) An Anthropic session lists search, edit, write_file, bash,
   run_typescript, skill (the edit tools before bash), bash's
   description says "use `edit`", the system prompt carries the edit
   line; a Mistral one the same; switched to the OpenAI provider (a
   /model switch, no restart) the next request lists apply_patch and
   no edit tool, bash says "use `apply_patch`", the prompt line too.
B) The tools run: write_file creates a file (parents made), edit
   replaces one match, all matches, keeps CRLF and the file's mode; a
   relative path lives in BEND_WORKDIR.
C) Every error is Vibe 2.25.8's text, byte for byte (the strings copied
   from vibe/core/tools/builtins/edit.py, write_file.py); when Vibe is
   installed (uv tool mistral-vibe), its own Edit/WriteFile tools run
   on the same cases and must say the same thing.
"""
import json, os, socket, stat, subprocess, sys, tempfile, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
import fake_provider as F  # noqa: E402
import wait  # noqa: E402

FAILS = []


def check(ok, what):
    print("%s %s" % ("ok  " if ok else "FAIL", what), flush=True)
    if not ok:
        FAILS.append(what)


# the request order (the wire lists the catalog reversed): the edit
# tools first, before bash
EDIT_ON = ["edit", "write_file", "skill", "run_typescript", "bash", "search_tool_functions"]
PATCH_ON = ["apply_patch", "skill", "run_typescript", "bash", "search_tool_functions"]
LINE_EDIT = "Edit files with `edit`, create them with `write_file`."
LINE_PATCH = "Edit and create files with `apply_patch`."
MANY = ("Found %d matches of the string to replace, but replace_all is false. To replace all "
        "occurrences, set replace_all to true. To replace only one occurrence, please provide more "
        "context to uniquely identify the instance.\nString: %s")


def vibe_python():
    p = os.path.expanduser("~/.local/share/uv/tools/mistral-vibe/bin/python")
    return p if os.path.exists(p) else None


VIBE_RUN = r'''
import asyncio, json, sys
from pathlib import Path
from vibe.core.tools.base import ToolError
from vibe.core.tools.builtins.edit import Edit, EditArgs, EditConfig
from vibe.core.tools.builtins.write_file import WriteFile, WriteFileArgs, WriteFileConfig
cases = json.loads(sys.argv[1]); cwd = Path(sys.argv[2])
async def one(name, args):
    try:
        if name == "edit":
            t = Edit.from_config(lambda: EditConfig())
            t.cwd = cwd
            a = EditArgs(**args)
        else:
            t = WriteFile.from_config(lambda: WriteFileConfig())
            t.cwd = cwd
            a = WriteFileArgs(**args)
        async for r in t.run(a):
            pass
        return "\n".join(f"{k}: {v}" for k, v in r.model_dump(mode="json").items())
    except ToolError as e:
        return "ERR " + str(e)
    except Exception as e:
        return "EXC %s: %s" % (type(e).__name__, e)
print(json.dumps([asyncio.run(one(n, a)) for n, a in cases]))
'''


def main():
    t0 = time.time()
    tmp = os.path.realpath(tempfile.mkdtemp(prefix="sb-edit-"))
    work = os.path.join(tmp, "work")
    os.makedirs(work)
    F.LOG = os.path.join(tmp, "fake.log")
    srv, fport = F.serve()
    base = "http://127.0.0.1:%d/v1" % fport
    models = os.path.join(tmp, "models.toml")
    open(models, "w").write(
        'version = 1\ndefault_model = "anthropic/claude-x"\n\n'
        '[providers.anthropic]\nname = "Anthropic"\napi = "anthropic"\nbase_url = "%s"\nkey_env = ""\n\n'
        '[providers.mistral]\nname = "Mistral"\napi = "openai-chat"\nbase_url = "%s"\nkey_env = ""\n\n'
        '[providers.openai]\nname = "OpenAI"\napi = "openai-responses"\nbase_url = "%s"\nkey_env = ""\n'
        % (base, base, base))
    cfg = os.path.join(tmp, "config.toml")
    open(cfg, "w").write('model = "anthropic/claude-x"\n')
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    session = os.path.join(tmp, "session.txt")
    env = {k: v for k, v in os.environ.items() if not k.startswith(("BEND_", "SB_", "BISE_"))}
    env.update(HOME=tmp, TMPDIR=tmp, XDG_STATE_HOME=os.path.join(tmp, "state"), BISE_MODELS_FILE=models,
               BEND_CONFIG=cfg, BEND_REPL_PORT=str(port), BEND_SESSION_FILE=session,
               BEND_WIRE_LOG=os.path.join(tmp, "wire.log"), BEND_MCP_INDEX=os.path.join(tmp, "mcp.txt"),
               BEND_SKILLS_INDEX=os.path.join(tmp, "sk.txt"), BEND_BG_ROOT=os.path.join(tmp, "bg"),
               BEND_WORKDIR=work, SB_AGENT="probe")
    log, err = os.path.join(tmp, "repl.log"), os.path.join(tmp, "repl.err")
    repl = subprocess.Popen([os.path.join(ROOT, "repl-live")], cwd=ROOT, env=env,
                            stdout=open(log, "w"), stderr=open(err, "w"))

    def turn(model, text):
        open(cfg, "w").write('model = "%s"\n' % model)
        sock = socket.create_connection(("127.0.0.1", port), timeout=120)
        sock.sendall(("run " + text.replace("\n", "\\n") + "\n").encode())
        f = sock.makefile("rb")
        out = []
        while True:
            line = f.readline()
            if not line:
                break
            out.append(line.decode(errors="replace").rstrip())
            if line.startswith(b"  obs: turn_done"):
                break
        sock.close()
        return out

    def recs():
        return [json.loads(l) for l in open(F.LOG)]

    try:
        def banner():
            assert repl.poll() is None, "FAIL the REPL exited: %s" % open(err).read()[-800:]
            return "REPL on" in open(log).read()
        wait.until(banner, 60, "the REPL banner")

        # ---- A: one toolset per provider ----
        def seen(r):
            return [t[0] for t in r["tools"]]

        def bash_desc(r):
            return dict((t[0], t[1]) for t in r["tools"]).get("bash", "")

        turn("anthropic/claude-x", "hello")
        r = recs()[-1]
        check(r["family"] == "anthropic" and seen(r) == EDIT_ON, "anthropic lists the edit toolset: %r" % seen(r))
        check("Not for editing files: use `edit`." in bash_desc(r) and "heredocs" not in bash_desc(r),
              "anthropic: bash says use `edit`, no heredoc invitation")
        check(LINE_EDIT in r["system"] and LINE_PATCH not in r["system"], "anthropic: the prompt names edit")
        tools = dict((t[0], t[1]) for t in r["tools"])
        check(tools.get("edit", "").startswith("Exact string replacement in a file. You must `read_file` first.")
              and tools.get("write_file", "").startswith("Create a new file. Errors if the file already exists"),
              "anthropic: Vibe's descriptions")
        turn("mistral/devstral", "hello again")
        r = recs()[-1]
        check(r["family"] == "openai-chat" and seen(r) == EDIT_ON and LINE_EDIT in r["system"],
              "mistral lists the edit toolset: %r" % seen(r))
        turn("openai/gpt-x", "and again")
        r = recs()[-1]
        check(r["family"] == "openai-responses" and seen(r) == PATCH_ON,
              "openai (a switch mid-session) lists apply_patch only: %r" % seen(r))
        check("Not for editing files: use `apply_patch`." in bash_desc(r)
              and LINE_PATCH in r["system"] and LINE_EDIT not in r["system"],
              "openai: bash and the prompt name apply_patch")
        turn("anthropic/claude-x", "back")
        r = recs()[-1]
        check(seen(r) == EDIT_ON and LINE_EDIT in r["system"], "back on anthropic: the edit toolset again")

        # ---- B + C: the tools on the disk, and the errors ----
        crlf = os.path.join(work, "crlf.txt")
        open(crlf, "wb").write(b"one\r\ntwo\r\nthree\r\n")
        os.chmod(crlf, 0o751)
        open(os.path.join(work, "many.txt"), "w").write("a x a x a\n")
        open(os.path.join(work, "bad.bin"), "wb").write(b"ok\xffno\n")
        os.makedirs(os.path.join(work, "adir"))
        open(os.path.join(work, "exists.txt"), "w").write("here\n")
        W = work
        cases = [
            ("write_file", {"file_path": "sub/new.txt", "content": "hé\nlo\n"},
             "file_path: %s/sub/new.txt\nbytes_written: 7\ncontent: hé\nlo\n" % W),
            ("edit", {"file_path": W + "/sub/new.txt", "old_string": "lo", "new_string": "LO"},
             "file: %s/sub/new.txt\nmessage: The file has been updated successfully.\nold_string: lo\nnew_string: LO" % W),
            ("edit", {"file_path": "crlf.txt", "old_string": "two\nthree", "new_string": "2\n3"},
             "file: %s/crlf.txt\nmessage: The file has been updated successfully.\nold_string: two\nthree\nnew_string: 2\n3" % W),
            ("edit", {"file_path": "many.txt", "old_string": "a", "new_string": "b", "replace_all": True},
             "file: %s/many.txt\nmessage: The file has been updated. All occurrences were successfully replaced\nold_string: a\nnew_string: b" % W),
            ("edit", {"file_path": "many.txt", "old_string": "x", "new_string": "y"}, "ERR " + MANY % (2, "x")),
            ("edit", {"file_path": "many.txt", "old_string": "zzz", "new_string": "y"},
             "ERR String to replace not found in file.\nString: zzz"),
            ("edit", {"file_path": "  ", "old_string": "a", "new_string": "b"}, "ERR File path cannot be empty"),
            ("edit", {"file_path": "many.txt", "old_string": "", "new_string": "b"},
             "ERR old_string cannot be empty. Use write_file to create new files."),
            ("edit", {"file_path": "many.txt", "old_string": "b", "new_string": "b"},
             "ERR No changes to make — old_string and new_string are identical"),
            ("edit", {"file_path": "nope.txt", "old_string": "a", "new_string": "b"},
             "ERR File does not exist: %s/nope.txt" % W),
            ("edit", {"file_path": "adir", "old_string": "a", "new_string": "b"},
             "ERR Path is not a file: %s/adir" % W),
            ("edit", {"file_path": "bad.bin", "old_string": "ok", "new_string": "b"},
             "ERR Cannot edit %s/bad.bin: file is not valid text (utf-8, byte 2)" % W),
            ("write_file", {"file_path": "exists.txt", "content": "x"},
             "ERR File '%s/exists.txt' already exists. Use edit to modify it." % W),
            ("write_file", {"file_path": " ", "content": "x"}, "ERR Path cannot be empty"),
            ("write_file", {"file_path": "big.txt", "content": "x" * 64001}, "ERR Content exceeds 64000 bytes limit"),
        ]
        marks = " ".join("[[%s: %s]]" % (n, json.dumps(a, ensure_ascii=False)) for n, a, _ in cases)
        n0 = len(recs())
        turn("anthropic/claude-x", marks)
        # the last request carries every call's result as the model
        # reads it: "tool <name> ok: <text>" / "tool <name> failed: <text>"
        results_seen = []
        for t in recs()[-1]["tool_texts"][-len(cases):]:
            head, _, text = t.partition(": ")
            results_seen.append(("ERR " if head.endswith(" failed") else "") + text)
        ok_disk = (open(os.path.join(work, "sub/new.txt")).read() == "hé\nLO\n"
                   and open(crlf, "rb").read() == b"one\r\n2\r\n3\r\n"
                   and stat.S_IMODE(os.stat(crlf).st_mode) == 0o751
                   and open(os.path.join(work, "many.txt")).read() == "b x b x b\n"
                   and not os.path.exists(os.path.join(work, "big.txt"))
                   and open(os.path.join(work, "exists.txt")).read() == "here\n")
        check(ok_disk, "the disk: write_file with parents, edit, CRLF and mode kept, replace_all, nothing overwritten")
        want = [w for _, _, w in cases]
        check(len(results_seen) == len(want), "one result per call: %d of %d" % (len(results_seen), len(want)))
        for (n, a, w), g in zip(cases, results_seen):
            check(g == w, "%s %s: %r" % (n, json.dumps(a)[:60], g[:160]))

        # C: Vibe itself on the same cases (fresh files), when installed
        vp = vibe_python()
        if vp:
            vw = os.path.join(tmp, "vibe")
            os.makedirs(os.path.join(vw, "adir"))
            open(os.path.join(vw, "many.txt"), "w").write("a x a x a\n")
            open(os.path.join(vw, "bad.bin"), "wb").write(b"ok\xffno\n")
            open(os.path.join(vw, "exists.txt"), "w").write("here\n")
            errs = [(n, a, w.replace(W, vw)) for n, a, w in cases if w.startswith("ERR ")
                    and "bad.bin" not in w]  # Vibe decodes a latin-1 file (charset_normalizer); bise refuses it
            vcases = [(n, dict(a, file_path=a["file_path"] if a["file_path"].strip() == "" or a["file_path"].startswith("/")
                               else a["file_path"])) for n, a, _ in errs]
            out = subprocess.run([vp, "-c", VIBE_RUN, json.dumps(vcases), vw], capture_output=True, text=True, timeout=120)
            try:
                vres = json.loads(out.stdout.strip().splitlines()[-1])
            except Exception:
                vres = None
            check(vres is not None, "Vibe's own tools ran: %s" % out.stderr[-300:])
            for (n, a, w), v in zip(errs, vres or []):
                check(v == w, "Vibe says the same for %s %s: %r" % (n, json.dumps(a)[:50], v[:120]))
        else:
            print("skip Vibe comparison (mistral-vibe not installed)")
    finally:
        repl.kill()
        srv.shutdown()
    if FAILS:
        sys.exit("FAIL %d: %s" % (len(FAILS), FAILS))
    print("PASS edit tools (%.1fs)" % (time.time() - t0))


if __name__ == "__main__":
    main()
