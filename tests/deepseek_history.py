#!/usr/bin/env python3
"""Offline DeepSeek replay regression against a real, throwaway repl-live.

The local fake rejects thinking blocks, missing native reasoning, and
assistant messages with neither content nor tool calls.
No keys are inherited, and both provider URLs point to this loopback server.
Pass --repl PATH to test an isolated build without installing it.
"""
import argparse
import http.server
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import threading

import wait

ROOT = Path(__file__).resolve().parent.parent


def run(repl_path):
    requests, failures = [], []

    class Handler(http.server.BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass

        def do_POST(self):
            body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            index = len(requests)
            requests.append(body)
            try:
                assert self.path == "/v1/chat/completions"
                assert not self.headers.get("Authorization"), "test inherited a key"
                if index:
                    assert body["model"] == "deepseek-flash"
                    for message in body["messages"]:
                        if message["role"] == "assistant":
                            assert message["content"] is None or isinstance(message["content"], str), \
                                "assistant content contains an unsupported thinking block"
                            assert message["content"] is not None or message.get("tool_calls"), (
                                "Invalid assistant message: content or tool_calls must be set")
                            assert "reasoning_content" in message
                            assert "<think>" not in (message["content"] or "")
                    assistants = [m for m in body["messages"] if m["role"] == "assistant"]
                    assert assistants[0]["content"] == "foreign answer"
                    assert assistants[0]["reasoning_content"] == ""
                    assert "foreign-sig" not in json.dumps(body)
                    assert "foreign thought" not in json.dumps(body)
                    if index >= 2:
                        assert assistants[1]["reasoning_content"] == " native first\n "
                    if index >= 3:
                        assert assistants[2]["content"] == ""
                        assert assistants[2]["reasoning_content"] == " reasoning-only\n "
                        assert not assistants[2].get("tool_calls")
                    if index in (4, 6):
                        last_call = next(m for m in reversed(assistants) if m.get("tool_calls"))
                        result = body["messages"][-1]
                        assert result["role"] == "tool"
                        assert result["tool_call_id"] == last_call["tool_calls"][0]["id"]
                        assert "offline-fixture" in result["content"]
                        assert last_call["reasoning_content"] == ("call reason" if index == 4 else "")
                    if index >= 5:
                        assert body["thinking"] == {"type": "disabled"}
                        assert "reasoning_effort" not in body
                    else:
                        assert body["reasoning_effort"] == "high"
                if index == 0:
                    assert body["model"] == "fake"
                    delta = {"content": [
                        {"type": "thinking", "thinking": [{"type": "text", "text": "foreign thought"}],
                         "signature": "foreign-sig"},
                        {"type": "text", "text": "foreign answer"},
                    ]}
                elif index == 1:
                    delta = {"reasoning_content": " native first\n ", "content": "native answer"}
                elif index == 2:
                    delta = {"reasoning_content": " reasoning-only\n ", "content": None}
                elif index in (3, 5):
                    delta = {"content": None, "tool_calls": [{
                        "index": 0, "id": "fake-call", "type": "function",
                        "function": {"name": "bash", "arguments": json.dumps({"arg": "printf offline-fixture"})},
                    }]}
                    if index == 3:
                        delta["reasoning_content"] = "call reason"
                elif index == 4:
                    delta = {"reasoning_content": "finish reason", "content": "finished high"}
                elif index == 6:
                    delta = {"content": "finished none"}
                else:
                    raise AssertionError("unexpected extra request")
            except AssertionError as error:
                failures.append(str(error) or "invalid request at index %d" % index)
                self.send_response(400 if failures[-1].startswith("Invalid assistant message:") else 422)
                self.end_headers()
                self.wfile.write(json.dumps({"error": failures[-1]}).encode())
                return
            response = ("data: " + json.dumps({"choices": [{"index": 0, "delta": delta}]})
                        + "\n\ndata: [DONE]\n\n").encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(response)))
            self.end_headers()
            self.wfile.write(response)

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        with tempfile.TemporaryDirectory(prefix="deepseek-history-") as directory:
            temp = Path(directory)
            base = "http://127.0.0.1:%d/v1" % server.server_address[1]
            models = temp / "models.toml"
            models.write_text("\n".join(
                '[providers.%s]\napi = "openai-chat"\nbase_url = "%s"\nkey_env = ""\n'
                'reasoning = true\nefforts = "none,high"\neffort = "high"\n'
                'context = 1000000\nmax_output = 256\n' % (provider, base)
                for provider in ("mistral", "deepseek")))
            cfg = temp / "config.toml"
            cfg.write_text('model = "mistral/fake"\n')
            with socket.socket() as reserve:
                reserve.bind(("127.0.0.1", 0))
                port = reserve.getsockname()[1]
            # Deliberately do not copy os.environ: no real credentials,
            # sign-ins, connector config, hub sockets or user session paths.
            env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": directory,
                   "TMPDIR": directory, "BISE_HOME": str(temp / "home"),
                   "XDG_STATE_HOME": str(temp / "state"), "BISE_MODELS_FILE": str(models),
                   "BEND_CONFIG": str(cfg), "BEND_REPL_PORT": str(port),
                   "BEND_SESSION_FILE": str(temp / "session.txt"),
                   "BEND_MCP_INDEX": str(temp / "mcp.txt"), "BEND_SKILLS_INDEX": str(temp / "skills.txt"),
                   "BEND_BG_ROOT": str(temp / "bg"), "BEND_RUN_DIR": str(temp / "run"),
                   "BEND_NO_TELEMETRY": "1"}
            with (temp / "repl.log").open("w") as log, (temp / "repl.err").open("w") as err:
                process = subprocess.Popen([str(repl_path)], cwd=temp, env=env, stdout=log, stderr=err)
                try:
                    def ready():
                        assert process.poll() is None, (temp / "repl.err").read_text()
                        return "REPL on" in (temp / "repl.log").read_text()

                    wait.until(ready, 60, "isolated REPL banner")

                    def turn(model, effort, text):
                        cfg.write_text('model = "%s"\nreasoning_effort = "%s"\n' % (model, effort))
                        with socket.create_connection(("127.0.0.1", port), timeout=60) as client:
                            client.sendall(("run " + text + "\n").encode())
                            with client.makefile("rb") as stream:
                                lines = []
                                for line in stream:
                                    lines.append(line.decode(errors="replace"))
                                    if line.startswith(b"  obs: turn_done"):
                                        break
                        assert not failures, failures
                        return "".join(lines)

                    turn("mistral/fake", "high", "seed a foreign signed answer")
                    turn("deepseek/deepseek-flash", "high", "native answer")
                    turn("deepseek/deepseek-flash", "high", "retain a reasoning-only partial answer")
                    high = turn("deepseek/deepseek-flash", "high", "use a fake tool")
                    assert "finished high" in high, high
                    none = turn("deepseek/deepseek-flash", "none", "use a fake tool without thinking")
                    assert "finished none" in none, none
                    assert len(requests) == 7, len(requests)
                finally:
                    process.terminate()
                    process.wait(timeout=10)
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=10)
    print("ok   DeepSeek history: 7 local requests; foreign signed, native and reasoning-only history, tool IDs, high/none")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repl", type=Path, default=ROOT / "repl-live")
    run(parser.parse_args().repl.resolve())
