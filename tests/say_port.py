"""A repl-live's say port in the tests, opened the way the hub opens it
(rust/switchboard/src/daemon/repl_starts.rs, repl.rs `say_hello`): a free
port in BEND_REPL_PORT, a fresh token in BISE_REPL_TOKEN, and every
connection's first line `hello <token>` (repl-core's serve_gate).

A REPL with no token would take any local bytes as his words: the hub's
own HTTP probe of an agent's processes (`HEAD / HTTP/1.0`, three lines)
was three model turns of provider_families' REPL (architect m_66084).

    sp = SayPort()
    env.update(sp.env())
    repl = subprocess.Popen([repl_live], env=env, ...)
    sock = sp.connect(timeout=60)    # hello written: say lines as before
"""
import secrets
import socket


def free_port():
    """A free loopback port (bound, then closed)."""
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def hello(token):
    """The first line of a connection to a REPL started with `token`."""
    return "hello %s\n" % token


class SayPort:
    """A say port and the token its connections open with."""

    def __init__(self, port=None, token=None):
        self.port = port or free_port()
        self.token = token or secrets.token_hex(16)

    def env(self):
        """What the REPL reads: its port and its token."""
        return {"BEND_REPL_PORT": str(self.port), "BISE_REPL_TOKEN": self.token}

    def connect(self, timeout=60):
        """A connection with its hello written: the REPL takes its lines."""
        sock = socket.create_connection(("127.0.0.1", self.port), timeout=timeout)
        sock.sendall(hello(self.token).encode())
        return sock
