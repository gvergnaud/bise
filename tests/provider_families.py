#!/usr/bin/env python3
"""BISE-153: the fake provider speaks the four wire families, and the
fixtures say what they claim (docs/research/providers.md §8).

A) Every family's renderer (SSE and whole) read back by provider_folds.py
   (written from the docs, not from the renderers): text, reasoning, two
   tool calls, usage; a mid-stream error.
B) tests/providers/<family>/: every file is in its index.json and folds
   to its `expect`; the fake-* files are what `fake_provider.py fixtures`
   writes today (not stale).
C) The HTTP server: each family's URL path, streamed and whole, scripted
   by markers in the conversation (bash calls one per request, think,
   error xN then the script, fixture), in each family's request shape.
D) A real repl-live on the fake through a custom provider (BISE_MODELS_FILE
   + BEND_CONFIG, the registry of BISE-141): a two-call bash turn on
   `anthropic` (streamed, with thinking and one 529 retried), then the
   config's model switched to an openai-chat provider for the next turn.
   A family the harness does not speak yet must fail cleanly. A gateway
   provider (headers_env + key_command, Claude Code's
   ANTHROPIC_CUSTOM_HEADERS + apiKeyHelper): its headers and the
   command's key on the wire; a failing command, no call and a clear line.
   A provider's idle_timeout_sec (provider-timeout) against a fake that
   waits 3 s before its first byte: 1 s times out, retries with a line
   that names the key and the provider's table (Ctrl+C keeps that text
   as the turn's error); 8 s waits and answers.
"""
import json, os, socket, subprocess, sys, tempfile, time, urllib.error, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
import fake_provider as F  # noqa: E402
import provider_folds as P  # noqa: E402

FAILS = []


def check(ok, what):
    print("%s %s" % ("ok  " if ok else "FAIL", what), flush=True)
    if not ok:
        FAILS.append(what)


def calls_of(t):
    return [(c["name"], c["args"]) for c in t["calls"]]


def part_a():
    t = F.FAKE_TURN
    for fam in F.FAMILIES:
        s = P.fold(fam, F.sse_bytes(fam, F.render(fam, t, "m", True, F.FAKE_BODY)))
        w = P.fold(fam, json.dumps(F.render(fam, t, "m", False, F.FAKE_BODY)), False)
        for kind, g in (("sse", s), ("whole", w)):
            check((g["text"], g["reasoning"], calls_of(g), g["error"]) ==
                  (t["text"], t["reasoning"], calls_of(t), None) and g["usage"],
                  "%s %s: text, reasoning, 2 tool calls, usage" % (fam, kind))
        e = P.fold(fam, F.sse_bytes(fam, F.render(fam, t, "m", True, {}, mid_error=True)))
        check(bool(e["error"]) and not e["calls"], "%s sse: a mid-stream error event" % fam)
    # the stream options change the stream the way the docs say
    no_usage = F.sse_bytes("openai-chat", F.render("openai-chat", t, "m", True, {}))
    check(b'"usage"' not in no_usage and b"data: [DONE]" in no_usage,
          "openai-chat: no usage chunk without stream_options.include_usage, then [DONE]")
    enc = F.render("openai-responses", t, "m", False, {"include": ["reasoning.encrypted_content"]})
    check(enc["output"][0].get("encrypted_content") == "fake-enc", "responses: encrypted_content when included")
    g = F.sse_bytes("gemini", F.render("gemini", t, "m", True))
    check(g.count(b"\r\n\r\n") == len(P.sse_events(g)) and b"thoughtSignature" in g,
          "gemini: CRLF events, the first functionCall has a thoughtSignature")


def part_b():
    for fam in F.FAMILIES:
        d = os.path.join(F.FIXTURES, fam)
        index = json.load(open(os.path.join(d, "index.json")))
        files = sorted(f for f in os.listdir(d) if f != "index.json")
        check(files == sorted(index), "%s: every fixture is in index.json (%d)" % (fam, len(files)))
        for f in files:
            raw = open(os.path.join(d, f), "rb").read()
            exp = index.get(f, {}).get("expect", {})
            try:
                got = P.fold(fam, raw, f.endswith(".sse"))
            except Exception as e:  # noqa: BLE001
                check(False, "%s/%s folds: %r" % (fam, f, e))
                continue
            bad = []
            for k, v in exp.items():
                g = calls_of(got) if k == "calls" else got[k]
                v = [(c["name"], c["args"]) for c in v] if k == "calls" else v
                if (bool(g) != v) if v is True else g != v:
                    bad.append("%s: %r != %r" % (k, g, v))
            check(not bad, "%s/%s says what index.json expects %s" % (fam, f, "; ".join(bad)))
        for f, (data, _) in F.fake_fixtures(fam).items():
            p = os.path.join(d, f)
            check(os.path.exists(p) and open(p, "rb").read() == data,
                  "%s/%s is fresh (else: fake_provider.py fixtures)" % (fam, f))


# ---- C: the server

SYS = "# Your role: task `t1`"


def body_for(fam, user, done=(), stream=True):
    """a request of family `fam`: system, the user message, then for each
    finished call (cmd, result) the assistant's call and its result"""
    if fam == "openai-chat":
        m = [{"role": "system", "content": SYS}, {"role": "user", "content": user}]
        for i, (cmd, res) in enumerate(done):
            m += [{"role": "assistant", "content": "", "tool_calls": [
                {"id": "c%d" % i, "type": "function", "function": {"name": "bash", "arguments": json.dumps({"arg": cmd})}}]},
                  {"role": "tool", "tool_call_id": "c%d" % i, "content": res}]
        m.append({"role": "user", "content": "<bise_state>x</bise_state>"})
        return {"model": "m", "stream": stream, "stream_options": {"include_usage": True}, "messages": m}
    if fam == "anthropic":
        m = [{"role": "user", "content": [{"type": "text", "text": user},
                                          {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "iVBO"}}]}]
        for i, (cmd, res) in enumerate(done):
            m += [{"role": "assistant", "content": [{"type": "tool_use", "id": "t%d" % i, "name": "bash", "input": {"arg": cmd}}]},
                  {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t%d" % i, "content": res}]}]
        return {"model": "m", "stream": stream, "system": [{"type": "text", "text": SYS}], "messages": m}
    if fam == "openai-responses":
        items = [{"role": "user", "content": [{"type": "input_text", "text": user}]}]
        for i, (cmd, res) in enumerate(done):
            items += [{"type": "function_call", "call_id": "c%d" % i, "name": "bash", "arguments": json.dumps({"arg": cmd})},
                      {"type": "function_call_output", "call_id": "c%d" % i, "output": res}]
        return {"model": "m", "stream": stream, "instructions": SYS, "input": items}
    cs = [{"role": "user", "parts": [{"text": user}]}]
    for cmd, res in done:
        cs += [{"role": "model", "parts": [{"functionCall": {"name": "bash", "args": {"arg": cmd}}}]},
               {"role": "user", "parts": [{"functionResponse": {"name": "bash", "response": {"output": res}}}]}]
    return {"systemInstruction": {"parts": [{"text": SYS}]}, "contents": cs}


def path_for(fam, stream=True):
    return {"openai-chat": "/v1/chat/completions", "anthropic": "/v1/messages",
            "openai-responses": "/v1/responses"}.get(
        fam, "/v1beta/models/gemini-x:" + ("streamGenerateContent?alt=sse" if stream else "generateContent"))


def call(port, fam, body, stream=True, path=None):
    req = urllib.request.Request("http://127.0.0.1:%d%s" % (port, path or path_for(fam, stream)),
                                 json.dumps(body).encode(), {"content-type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            return r.status, dict(r.headers), r.read()
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read()


def part_c():
    log = os.path.join(tempfile.mkdtemp(prefix="sb-fams-"), "fake.log")
    F.LOG = log
    srv, port = F.serve()
    try:
        for fam in F.FAMILIES:
            for stream in (True, False):
                user = "[[think: plan it]] go [[bash: echo one]] [[bash: echo two]] (%s %s)" % (fam, stream)
                seen = []
                done = []
                for step in range(3):
                    st, hdr, raw = call(port, fam, body_for(fam, user, done, stream), stream)
                    t = P.fold(fam, raw, stream)
                    seen.append((st, calls_of(t), t["text"], t["reasoning"]))
                    if t["calls"]:
                        done.append((t["calls"][0]["args"]["arg"], "out-%d" % step))
                sse = "event-stream" in hdr.get("content-type", hdr.get("Content-Type", ""))
                want = [(200, [("bash", {"arg": "echo one"})], "", "plan it"),
                        (200, [("bash", {"arg": "echo two"})], "", "plan it"),
                        (200, [], "done: out-1", "plan it")]
                check(seen == want and sse == stream,
                      "%s %s: two bash calls, one per request, then done (think each time)%s"
                      % (fam, "sse" if stream else "whole", "" if seen == want else " %r" % seen))
            # errors: 429 x2 then 529/503, then the script goes on
            user = "[[error: 429 x2 retry=3]] [[error: overloaded]] [[error: stream]] hello (%s)" % fam
            got = [call(port, fam, body_for(fam, user)) for _ in range(5)]
            over = 529 if fam == "anthropic" else 503
            sts = [g[0] for g in got]
            ra = [g[1].get("retry-after", g[1].get("Retry-After")) for g in got]
            check(sts == [429, 429, over, 200, 200] and ra[:3] == ["3", "3", "1"],
                  "%s: 429 x2 (Retry-After 3), overloaded %d, then the stream: %r %r" % (fam, over, sts, ra))
            mid = P.fold(fam, got[3][2])
            last = P.fold(fam, got[4][2])
            check(bool(mid["error"]) and last["text"].startswith("ack: ") and "hello" in last["text"],
                  "%s: a mid-stream error event, then the reply" % fam)
            # a fixture, byte for byte
            st, _, raw = call(port, fam, body_for(fam, "[[fixture: fake-tool-call]] x"))
            check(st == 200 and raw == open(os.path.join(F.FIXTURES, fam, "fake-tool-call.sse"), "rb").read(),
                  "%s: [[fixture: fake-tool-call]] replays the file" % fam)
            st, _, raw = call(port, fam, body_for(fam, "[[fixture: fake-rate-limit]] x"))
            check(st == 429, "%s: [[fixture: fake-rate-limit]] answers its status 429" % fam)
        # Gemini's stream without alt=sse: one JSON array
        st, _, raw = call(port, "gemini", body_for("gemini", "hi"), path="/v1beta/models/g:streamGenerateContent")
        check(st == 200 and isinstance(json.loads(raw), list) and P.fold("gemini", raw, False)["text"] == "ack: hi",
              "gemini: streamGenerateContent without alt=sse is a JSON array")
        # the old whole chat reply, as e2e and the tmux tests read it
        st, _, raw = call(port, "openai-chat", {"model": "m", "messages": [
            {"role": "system", "content": SYS}, {"role": "user", "content": "[[bash: ls]]"}]}, False)
        d = json.loads(raw)
        m = d["choices"][0]["message"]
        check(d["object"] == "chat.completion" and m["content"] == "" and m["tool_calls"][0]["id"] == "call_1_0"
              and d["choices"][0]["finish_reason"] == "tool_calls", "openai-chat whole: the old reply shape")
        recs = [json.loads(l) for l in open(log)]
        fams = {r["family"] for r in recs}
        check(fams == set(F.FAMILIES) and all(r["agent"] == "t1" for r in recs),
              "the log names each request's family and agent")
        check(any(r["family"] == "anthropic" and r["images"] and r["images"][0].startswith("data:image/png;base64,")
                  for r in recs), "the log lists an Anthropic request's images")
    finally:
        srv.shutdown()


# ---- D: repl-live on the fake, through a custom provider

def part_d():
    tmp = tempfile.mkdtemp(prefix="sb-fams-repl-")
    F.LOG = os.path.join(tmp, "fake.log")
    srv, fport = F.serve()
    models = os.path.join(tmp, "models.toml")
    base = "http://127.0.0.1:%d/v1" % fport
    open(models, "w").write(
        'version = 1\ndefault_model = "fake/claude-x"\n\n'
        '[providers.fake]\nname = "Fake"\napi = "anthropic"\nbase_url = "%s"\nkey_env = ""\n\n'
        '[providers.fakeoai]\nname = "Fake OpenAI"\napi = "openai-chat"\nbase_url = "%s"\nkey_env = ""\n\n'
        '[providers.fakegem]\nname = "Fake Gemini"\napi = "gemini"\nbase_url = "%s"\nkey_env = ""\n\n'
        '[providers.fakegw]\nname = "Fake Gateway"\napi = "anthropic"\nbase_url = "%s"\nkey_env = ""\n'
        'headers_env = "FAKE_GW_HEADERS"\nkey_command = "printf \'gw-%%s\' tok; echo"\n\n'
        '[providers.fakegw.headers]\n"source" = "config"\n"x-team" = "platform"\n'
        '"x-quoted" = "hello \\"world\\""\n\n'
        '[models."fakegw/native"]\nheaders_env = ""\n'
        '[models."fakegw/native".headers]\n"source" = "model"\n\n'
        '[models."fakegw/missing"]\nheaders_env = "UNSET_GW_HEADERS"\n\n'
        '[models."fakegw/empty"]\nheaders_env = "EMPTY_GW_HEADERS"\n\n'
        '[models."fakegw/broken"]\nkey_command = "echo nope; exit 3"\n\n'
        '[providers.fakeslow]\nname = "Fake Slow"\napi = "openai-chat"\nbase_url = "%s"\nkey_env = ""\n'
        'idle_timeout_sec = 1\n\n'
        '[providers.fakepatient]\nname = "Fake Patient"\napi = "openai-chat"\nbase_url = "%s"\nkey_env = ""\n'
        'idle_timeout_sec = 8\n'
        % (base, base, base, base, base, base))
    cfg = os.path.join(tmp, "config.toml")
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    session = os.path.join(tmp, "session.txt")
    env = {k: v for k, v in os.environ.items() if not k.startswith(("BEND_", "SB_", "BISE_"))}
    env.update(HOME=tmp, XDG_STATE_HOME=os.path.join(tmp, "state"), BISE_MODELS_FILE=models, BEND_CONFIG=cfg,
               BEND_REPL_PORT=str(port), BEND_SESSION_FILE=session, BEND_WIRE_LOG=os.path.join(tmp, "wire.log"),
               BEND_MCP_INDEX=os.path.join(tmp, "mcp.txt"), BEND_SKILLS_INDEX=os.path.join(tmp, "sk.txt"),
               BEND_BG_ROOT=os.path.join(tmp, "bg"), SB_AGENT="probe", TMPDIR=tmp,
               FAKE_GW_HEADERS="source: bise\norg-id : 2\nnot a header", EMPTY_GW_HEADERS="   ")
    log, err = os.path.join(tmp, "repl.log"), os.path.join(tmp, "repl.err")
    repl = subprocess.Popen([os.path.join(ROOT, "repl-live")], cwd=ROOT, env=env,
                            stdout=open(log, "w"), stderr=open(err, "w"))

    # the interrupt flag the TUI writes on Ctrl+C (side dir: $TMPDIR)
    flag = os.path.join(tmp, "bend-interrupt-%d.txt" % port)

    def turn(model, text, ctrl_c_on=None):
        open(cfg, "w").write('model = "%s"\n' % model)
        sock = socket.create_connection(("127.0.0.1", port), timeout=60)
        sock.sendall(("run " + text + "\n").encode())
        f = sock.makefile("rb")
        out = []
        while True:
            line = f.readline()
            if not line:
                break
            out.append(line.decode(errors="replace").rstrip())
            if ctrl_c_on and ctrl_c_on in out[-1]:
                open(flag, "w").write("1")
                ctrl_c_on = None
            if line.startswith(b"  obs: turn_done"):
                break
        sock.close()
        return out

    try:
        t0 = time.time()
        while "REPL on" not in open(log).read():
            if repl.poll() is not None or time.time() - t0 > 60:
                sys.exit("FAIL no REPL banner: %s" % open(err).read()[-500:])
            time.sleep(0.1)
        out = turn("fake/claude-x", "[[error: overloaded retry=0]] [[think: two steps]] [[bash: echo a1]] [[bash: echo a2]]")
        recs = [json.loads(l) for l in open(F.LOG)]
        text = open(session).read()
        said = [l for l in out if "obs: assistant:" in l]
        check([r["status"] for r in recs] == [529, 200, 200, 200] and all(
            r["family"] == "anthropic" and r["stream"] and r["path"] == "/v1/messages" for r in recs),
            "anthropic via [providers.fake]: streamed to /v1/messages, the 529 retried: %r"
            % [(r["family"], r["status"]) for r in recs])
        check("tool bash ok: a1" in text and "tool bash ok: a2" in text and said and "done: " in said[-1],
              "anthropic: two bash calls, then the last reply: %r" % (said[-1:] if said else out[-5:]))
        n = len(recs)
        out = turn("fakeoai/m2", "[[bash: echo b1]]")
        recs = [json.loads(l) for l in open(F.LOG)][n:]
        check([r["family"] for r in recs] == ["openai-chat"] * 2 and all(r["stream"] for r in recs)
              and "tool bash ok: b1" in open(session).read(),
              "a config.toml model switch goes to openai-chat at the next call, streamed (BISE-144): %r"
              % [(r["family"], r["stream"]) for r in recs])
        n += len(recs)
        out = turn("fakegem/g", "hello")
        recs = [json.loads(l) for l in open(F.LOG)][n:]
        errs = [l for l in out if "rror" in l or "not supported" in l.lower() or "obs: assistant" in l]
        check(repl.poll() is None and (recs and recs[0]["family"] == "gemini" or errs),
              "a gemini model: a reply, or a clean error before BISE-148: %r" % (errs[-1:] or out[-2:]))
        n += len(recs)
        out = turn("fakegw/c", "hello gateway")
        recs = [json.loads(l) for l in open(F.LOG)][n:]
        h = recs[0]["headers"] if recs else {}
        check(len(recs) == 1 and recs[0]["status"] == 200 and h.get("x-api-key") == "gw-tok"
              and h.get("authorization") == "Bearer gw-tok"
              and h.get("source") == "bise" and h.get("x-team") == "platform" and h.get("org-id") == "2" and "not a header" not in json.dumps(h),
              "a gateway: key_command's key (trimmed, also as a bearer token) and headers_env's headers on the wire: %r" % h)
        n += len(recs)
        out = turn("fakegw/native", "hello native headers")
        recs = [json.loads(l) for l in open(F.LOG)][n:]
        h = recs[0]["headers"] if recs else {}
        check(len(recs) == 1 and h.get("source") == "model" and h.get("x-team") == "platform"
              and h.get("x-quoted") == 'hello "world"',
              "native headers need no environment; model overrides and escaped values reach the server: %r" % h)
        n += len(recs)
        for model, variable in [("missing", "UNSET_GW_HEADERS"), ("empty", "EMPTY_GW_HEADERS")]:
            out = turn("fakegw/" + model, "hello missing headers")
            recs = [json.loads(l) for l in open(F.LOG)][n:]
            check(not recs and any(variable in line and "not set or is empty" in line for line in out),
                  "an unset or blank headers_env stops before HTTP and names the variable: " + model)
        out = turn("fakegw/broken", "hello broken")
        recs = [json.loads(l) for l in open(F.LOG)][n:]
        check(repl.poll() is None and not recs and any("key_command failed" in l for l in out),
              "a failing key_command: no call, a line that names it: %r" % out[-3:])
        # provider-timeout: the same slow server (3 s before the first
        # byte), two idle_timeout_sec; the first ends with a Ctrl+C (ten
        # attempts with their backoff would take minutes)
        t0 = time.time()
        out = turn("fakeslow/m", "[[slow: 3]] hello slow", ctrl_c_on="provider_retry")
        took = time.time() - t0
        retry = [l for l in out if "provider_retry" in l]
        key = "(timeout) — a slow model or gateway? raise idle_timeout_sec under [providers.fakeslow] in ~/.bise/config.toml"
        check(retry and "no answer from 127.0.0.1:%d in 1 s %s" % (fport, key) in retry[0],
              "idle_timeout_sec = 1: the read times out after 1 s and the retry line names the key: %r"
              % (retry[:1] or out[-3:]))
        stop = [l for l in out if "stopped retrying" in l]
        check(stop and key in stop[-1] and took < 15 and repl.poll() is None,
              "Ctrl+C during the retries: the turn's error keeps the timeout text (%.1fs): %r"
              % (took, stop[-1:] or out[-3:]))
        n = len([json.loads(l) for l in open(F.LOG)])
        t0 = time.time()
        out = turn("fakepatient/patient", "[[slow: 3]] hello patient")
        took = time.time() - t0
        # The interrupted slow request can finish after the next turn starts.
        recs = [json.loads(l) for l in open(F.LOG)][n:]
        recs = [r for r in recs if r.get("model") == "patient"]
        said = [l for l in out if "obs: assistant:" in l]
        check(not [l for l in out if "provider_retry" in l] and said and "ack: " in said[-1]
              and [r["status"] for r in recs] == [200] and took >= 3,
              "idle_timeout_sec = 8: the same 3 s wait answers, no retry (%.1fs): %r"
              % (took, said[-1:] or out[-3:]))
    finally:
        repl.kill()
        srv.shutdown()


def main():
    t0 = time.time()
    part_a()
    part_b()
    part_c()
    if "--no-repl" not in sys.argv:
        part_d()
    if FAILS:
        sys.exit("FAIL %d: %s" % (len(FAILS), FAILS))
    print("PASS provider families (%.1fs)" % (time.time() - t0))


if __name__ == "__main__":
    main()
