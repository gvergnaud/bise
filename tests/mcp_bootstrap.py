#!/usr/bin/env python3
"""The connector index at the live REPL's start (runtime/mcp.bend bootstrap).

The REPL GETs the connectors' bootstrap with the mistral provider's key
(MISTRAL_API_KEY, whatever the chat model) and writes the index lines to
$BEND_MCP_INDEX. A fake gateway (BEND_MCP_BOOTSTRAP_URL) checks:

1. 500 then 200: one retry, the index is written from the 200.
2. 500 twice: the index on disk is kept (it was overwritten with "" before:
   the gateway answers 500 now and then, and every connector was gone).
3. no Mistral key: no request, the index is kept, the log says why.
4. a chat model of another provider (foundry) still uses the Mistral key.
"""
import http.server, json, os, socket, subprocess, sys, tempfile, threading, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
import bise_env  # noqa: E402
import wait  # noqa: E402
NL = chr(10)
OLD = "old-cid old tool : #kept" + NL

# bise's internal and test variables, and the user's home and keys
PRIVATE = bise_env.NOT_INHERITED + ("BISE_HOME", "MISTRAL_API_KEY", "ANTHROPIC_FOUNDRY_API_KEY", "BEND_MODEL")

CONNECTORS = {"connectors": [{
    "id": "c-1", "name": "Slack", "status": {"is_ready": True},
    "tools": [{"name": "search_messages", "description": "Search" + NL + "messages",
               "inputSchema": {"type": "object"}}]}]}


class Gateway:
    """answers the queued statuses in order (then 200); records the auth headers"""

    def __init__(self):
        self.statuses, self.auth = [], []
        gw = self

        class H(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                gw.auth.append(self.headers.get("Authorization"))
                code = gw.statuses.pop(0) if gw.statuses else 200
                body = json.dumps(CONNECTORS if code == 200 else {"detail": "Internal Server Error"})
                self.send_response(code)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body.encode())

            def log_message(self, *a):
                pass

        self.srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()
        self.url = "http://127.0.0.1:%d/v1/connectors/bootstrap" % self.srv.server_address[1]


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def start(tmp, name, gw, extra):
    """a repl-live until its '[mcp]' line; returns (index content, log)"""
    d = os.path.join(tmp, name)
    os.makedirs(d)
    index = os.path.join(d, "mcp-index.txt")
    open(index, "w").write(OLD)
    env = {k: v for k, v in os.environ.items() if k not in PRIVATE}
    env.update(HOME=d, BEND_MCP_INDEX=index, BEND_MCP_BOOTSTRAP_URL=gw.url,
               BEND_SKILLS_INDEX=os.path.join(d, "sk.txt"),
               BEND_PLUGINS_STATE=os.path.join(d, "plugins.json"),
               BEND_CONFIG=os.path.join(d, "config.toml"),
               BEND_SESSION_FILE=os.path.join(d, "session.txt"),
               BEND_PROVIDER_URL="http://127.0.0.1:9/v1/chat/completions",
               BEND_REPL_PORT=str(free_port()), **extra)
    log = os.path.join(d, "repl.log")
    repl = subprocess.Popen([os.path.join(ROOT, "repl-live")], cwd=d, env=env,
                            stdout=open(log, "w"), stderr=subprocess.STDOUT)
    try:
        def done():
            """the bootstrap's last line: the index written, or kept"""
            assert repl.poll() is None, "FAIL %s: the REPL exited: %s" % (name, open(log).read()[-500:])
            text = open(log).read()
            return "[mcp] connector index written" in text or "the connector index on disk is kept" in text
        wait.until(done, 60, lambda: "%s: the bootstrap's [mcp] line: %s" % (name, open(log).read()[-500:]))
        return open(index).read(), open(log).read()
    finally:
        repl.kill()
        repl.wait()


def main():
    fails = []

    def check(name, ok, detail=""):
        print("%s %s" % ("ok  " if ok else "FAIL", name))
        if not ok:
            fails.append("%s %s" % (name, detail))

    key = {"MISTRAL_API_KEY": "mistral-test-key", "BEND_MODEL": "mistral-small-latest"}
    with tempfile.TemporaryDirectory() as tmp:
        gw = Gateway()
        gw.statuses = [500]
        idx, log = start(tmp, "retry", gw, key)
        check("a 500 then a 200: the index comes from the 200",
              idx.startswith("c-1 Slack search_messages : #Search messages | input: ")
              and "[mcp] connector index written" in log and len(gw.auth) == 2, repr((idx, gw.auth)))
        check("the Mistral key is the bearer", gw.auth[-1:] == ["Bearer mistral-test-key"], repr(gw.auth))

        gw.auth, gw.statuses = [], [500, 500]
        idx, log = start(tmp, "down", gw, key)
        check("a 500 twice: the index on disk is kept",
              idx == OLD and "bootstrap failed twice" in log and len(gw.auth) == 2,
              repr((idx, log[-300:])))

        gw.auth, gw.statuses = [], []
        idx, log = start(tmp, "nokey", gw, {"BEND_MODEL": "mistral-small-latest"})
        check("no Mistral key: no request, the index is kept, the log says so",
              idx == OLD and gw.auth == [] and "no Mistral key" in log,
              repr((idx, gw.auth, log[-300:])))

        gw.auth, gw.statuses = [], []
        idx, log = start(tmp, "foundry", gw, {"MISTRAL_API_KEY": "mistral-test-key",
                                              "ANTHROPIC_FOUNDRY_API_KEY": "foundry-test-key",
                                              "BEND_MODEL": "opus-5.5"})
        check("a foundry chat model: the connectors still use the Mistral key",
              idx.startswith("c-1 Slack ") and gw.auth == ["Bearer mistral-test-key"], repr((idx, gw.auth)))
        gw.srv.shutdown()
    if fails:
        sys.exit("FAIL %s" % fails)
    print("PASS mcp_bootstrap")


if __name__ == "__main__":
    main()
