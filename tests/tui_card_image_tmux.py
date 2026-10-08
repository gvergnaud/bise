"""An image pasted in the terminal reaches the model as an image block, not
as its path or the bare `[Image #1]` (docs/images.md), on both ways the
user hands one to an agent, in a real terminal (tmux) against the fake
provider (Anthropic family: the user's own provider shape, foundry/opus):

1. the thread: Cmd+V on an image (the terminal's empty bracketed paste:
   the clipboard image) in main's composer, sent: main's request carries
   an `image` block with the PNG's base64; the feed shows the chip
   `▣ clipboard`, never the marker.
2. a card answer: task t3 asks main, main escalates with `sb card --for`,
   the user opens the card (ctrl+1), pastes an image and answers: t3's
   request carries the image block, and the answer's text keeps the
   name the model reads (`[Image #1]`), never the stored path alone.

python3 -u tests/tui_card_image_tmux.py
"""
import base64
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, tmux, wait_until, MAIN_IDLE  # noqa: E402

# 1x1 PNG
PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==")
B64 = base64.b64encode(PNG).decode()
CTRL = "\x1b[%d;5u"   # ctrl+digit, the kitty keyboard protocol's form


def bodies(path):
    try:
        return [json.loads(l) for l in open(path)]
    except FileNotFoundError:
        return []


def image_blocks(body):
    """Each user message's image blocks (Anthropic), with the text parts
    next to them: [(texts, [base64 data])]."""
    out = []
    for m in body.get("messages", []):
        c = m.get("content")
        if m.get("role") != "user" or not isinstance(c, list):
            continue
        imgs = [(b.get("source") or {}).get("data", "") for b in c
                if isinstance(b, dict) and b.get("type") == "image"]
        if imgs:
            out.append(([b.get("text", "") for b in c if isinstance(b, dict) and b.get("type") == "text"], imgs))
    return out


def asked_by(path, needle):
    """The Anthropic request bodies whose text holds `needle`."""
    return [r["body"] for r in bodies(path)
            if r.get("family") == "anthropic" and needle in json.dumps(r.get("body"))]


def check_image(path, needle, what):
    """The newest request holding `needle` carries one image block with
    the PNG, wrapped by its name; never the path as the only thing."""
    def found():
        for body in reversed(asked_by(path, needle)):
            got = image_blocks(body)
            if got:
                return got
        return None
    got = wait_until(found, 60, lambda: "%s: no request with an image block (requests holding %r: %d)"
                     % (what, needle, len(asked_by(path, needle))))
    texts, imgs = got[-1]
    assert imgs == [B64], "%s: the image is the PNG: %r" % (what, [i[:40] for i in imgs])
    joined = "".join(texts)
    assert '<image name=[Image #1]' in joined or '[Image #1]' in joined, "%s: its name next to it: %r" % (what, texts)
    return texts


def main():
    fake_bodies = os.path.join(os.environ.get("TMPDIR", "/tmp"), "card-image-%d.jsonl" % os.getpid())
    E = e2e.Env(fake_env={"FAKE_BODIES": fake_bodies})
    clip = os.path.join(E.tmp, "clip.png")
    with open(clip, "wb") as f:
        f.write(PNG)
    # the Anthropic family, as the user's provider (foundry: api anthropic)
    port = E.env["BEND_PROVIDER_URL"].split(":")[2].split("/")[0]
    os.makedirs(os.path.join(E.env["HOME"], ".bise"), exist_ok=True)
    with open(os.path.join(E.env["HOME"], ".bise", "config.toml"), "a") as f:
        f.write('model = "fake/claude-x"\n\n[providers.fake]\nname = "Fake"\napi = "anthropic"\n'
                'base_url = "http://127.0.0.1:%s/v1"\nkey_env = ""\n' % port)
    del E.env["BEND_PROVIDER_URL"], E.env["BEND_MODEL"]
    env = "BISE_CTRL_DIGITS=1 BEND_CLIPBOARD_IMAGE_FILE=%s" % clip
    try:
        with tui_session(150, 42, env, E=E) as t:
            t.wait("bise :*")
            t.wait_re(MAIN_IDLE)
            # 1. the thread: Cmd+V (an empty bracketed paste) on an image
            t.typed("look ")
            tmux("send-keys", "-t", t.name, "-l", "\x1b[200~\x1b[201~")
            t.wait_re(r"▣ 1\s+clipboard")
            t.typed(" which color?")
            t.keys("Enter")
            sc = t.wait("ack: look [Image #1] which color?", 60)
            assert "▣ clipboard" in sc and "<image name=" not in sc and ".b64" not in sc, sc
            check_image(fake_bodies, "which color?", "main's thread")
            print("ok: a pasted image reaches main's model as an image block")
            t.wait_re(MAIN_IDLE)
            # 2. a card answer: t3 asks main, main escalates it to the user
            c = e2e.Client(os.path.join(E.state, "hub.sock"))
            c.say('/new t3: {{bash: sb send main --expect-reply "which logo, the red or the blue?"}}')
            c.wait(lambda: any(l.startswith("sb msg-in : t3 m_") for l in c.lines("main")), 120, "t3's question in main")
            msg = next(l.split()[4] for l in c.lines("main") if l.startswith("sb msg-in : t3 m_"))
            c.wait_idle("main", "t3")
            c.say('[[bash: sb card --for %s "which logo, the red or the blue?"]]' % msg)
            c.wait(lambda: any(cd["kind"] == "question" and cd["agent"] == "t3" for cd in c.cards()), 60, "t3's card")
            c.wait_idle("main")
            t.wait("waiting for you", 30)
            t.typed(CTRL % ord("1"))
            t.wait("your answer")
            t.typed("this one ")
            tmux("send-keys", "-t", t.name, "-l", "\x1b[200~\x1b[201~")
            t.wait_re(r"▣ 1\s+clipboard")
            t.typed(" please")
            t.keys("Enter")
            c.wait(lambda: not c.cards(), 30, "the card answered")
            check_image(fake_bodies, "please", "t3's card answer")
            print("ok: an image in a card answer reaches the asking agent's model as an image block")
    finally:
        if os.path.exists(fake_bodies):
            os.remove(fake_bodies)
    print("PASS tui card image")


if __name__ == "__main__":
    run(main)
