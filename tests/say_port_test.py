#!/usr/bin/env python3
"""tests/say_port.py's hello is the hub's (tests/fixtures/say_hello.json,
which rust/switchboard/src/daemon/repl.rs's test reads too), and a SayPort
gives the REPL a fresh token each time.

python3 tests/say_port_test.py
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from say_port import SayPort, hello  # noqa: E402


def main():
    fx = json.load(open(os.path.join(HERE, "fixtures", "say_hello.json")))
    assert hello(fx["token"]) == fx["line"], (hello(fx["token"]), fx["line"])
    a, b = SayPort(port=1), SayPort(port=1)
    assert a.token and a.token != b.token, (a.token, b.token)
    assert a.env() == {"BEND_REPL_PORT": "1", "BISE_REPL_TOKEN": a.token}, a.env()
    print("say_port_test: ok")


if __name__ == "__main__":
    main()
