"""A scripted provider for the switchboard tests, one server for the four
wire families (BISE-153). BEND_PROVIDER_URL, or a `[providers.fake]`
base_url, points here; the URL path picks the family:

  .../messages                          anthropic         (Messages API)
  .../responses                         openai-responses  (Responses API)
  .../models/<m>:generateContent        gemini            (whole reply)
  .../models/<m>:streamGenerateContent  gemini            (?alt=sse: SSE, else a JSON array)
  anything else                         openai-chat       (Chat Completions)

A request with "stream": true (Gemini: the stream path) gets a
Server-Sent Events reply shaped like the real API's; otherwise the whole
JSON reply (the old tests: openai-chat, not streamed, unchanged).

The script lives in the conversation itself (the last real user message:
the injected <bise_state> block is not one):
- `[[bash: CMD]]` markers (also `[[edit: JSON]]`, `[[write_file: JSON]]`:
  JSON args) make the agent call its bash tool with each
  CMD, in order, one call per model request; when there is no `[[...]]`
  marker, `{{bash: CMD}}` markers are used instead (so main's message can
  carry the script of a task's brief); `[[bash: CMD @@ DESC]]` sends the
  call's description too (BISE-223);
- once every marker ran (or there is none), the agent answers
  "done: <last tool result>" or "ack: <the message>";
- `[[think: TEXT]]`: every reply to that message starts with reasoning
  TEXT (Anthropic thinking + signature, reasoning_content, a Responses
  reasoning item, Gemini thought parts + thoughtSignature);
- `[[error: KIND]]` / `[[error: KIND xN]]`: the first N requests for
  that message fail (N = 1), then the script goes on. KIND: 429, 500,
  overloaded (Anthropic 529, the others 503), stream (a 200 stream that
  breaks with the family's error event; a whole reply gets a 500),
  badname (a final 400: the API refusing a tool name, BISE-293).
  Several markers fail in their order. `retry=S` sets Retry-After (1);
- `[[fixture: NAME]]`: the first request for that message is answered
  with the recorded file tests/providers/<family>/NAME.sse (streamed) or
  NAME.json (whole), or NAME.<status>.json (an error with that status),
  byte for byte.

A request whose tools or history calls carry a tool name off the API's
pattern (OpenAI ^[a-zA-Z0-9_-]+$, 64 chars; Anthropic 128) gets the
API's own 400, as the real ones do (BISE-293).

A key (Authorization, x-api-key) holding "bad" gets a 401, one holding
"broke" a 402 (BISE-266: the first run's key check), until the file
$FAKE_CREDIT exists (credit added, BISE-291); "broke-url" says so
with OpenAI's words, a url and `."` in them (BISE-287).

Speech to text (BISE-298): .../audio/transcriptions, .../speech-to-text
and .../listen answer {"text": $FAKE_STT_TEXT} ("" by default); a key
holding "bad" gets a 401, "broke" a 402, "down" a 503.

Each request is logged to $FAKE_LOG (one JSON line: agent, last user
message, reply as {content, tool_calls}, images, family, stream, status)
for the assertions.

Not a server: `fake_provider.py render FAMILY [--whole] TURN.json`
prints what the server would send for a turn ({"text", "reasoning",
"calls": [{"name", "args"}], "error"}); `fake_provider.py bend FILE`
prints FILE as a Bend string literal (a LAWS fixture).
Shapes: docs/research/providers.md §8 (the docs each one follows).
"""
import base64
import hashlib
import http.server
import json
import os
import re
import socketserver
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
FIXTURES = os.path.join(HERE, "providers")
LOG = os.environ.get("FAKE_LOG", "/tmp/sb-fake.log")
FAMILIES = ("anthropic", "openai-chat", "openai-responses", "gemini")
MARK = re.compile(r"\[\[(bash|skill|edit|write_file|apply_patch): (.*?)\]\]", re.S)
INNER = re.compile(r"\{\{(bash): (.*?)\}\}", re.S)
THINK = re.compile(r"\[\[think: (.*?)\]\]", re.S)
ERROR = re.compile(r"\[\[error: (\w+)(?: x(\d+))?(?: retry=(\d+))?\]\]")
FIXTURE = re.compile(r"\[\[fixture: ([\w.-]+)\]\]")
USAGE = {"input": 10, "cached": 2, "output": 5, "reasoning": 3}


# ---------------------------------------------------------------- requests
# Each family's request becomes one neutral conversation: a list of
# {"role": system|user|assistant|tool, "text", "calls" (tool calls made),
#  "images" (data urls)}.

def text_of(content):
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "\n".join(b.get("text", "") for b in content if isinstance(b, dict))
    return ""


def msg(role, text="", calls=0, images=None):
    return {"role": role, "text": text, "calls": calls, "images": images or []}


def agents_md_of(conv):
    """the AGENTS.md block of the system prompt (BISE-232), or """""
    sysm = "\n".join(m["text"] for m in conv if m["role"] == "system")
    i = sysm.find("# AGENTS.md instructions")
    if i < 0:
        return ""
    j = sysm.find("</INSTRUCTIONS>", i)
    return sysm[i:j + len("</INSTRUCTIONS>")] if j >= 0 else sysm[i:]


def conv_openai_chat(body):
    out = []
    for m in body.get("messages", []):
        c = m.get("content")
        imgs = [p.get("image_url", {}).get("url", "") for p in (c if isinstance(c, list) else [])
                if isinstance(p, dict) and p.get("type") == "image_url"]
        out.append(msg(m.get("role", "user"), text_of(c), len(m.get("tool_calls") or []), imgs))
    return out


def conv_anthropic(body):
    out = []
    sysm = body.get("system")
    if sysm:
        out.append(msg("system", text_of(sysm)))
    for m in body.get("messages", []):
        c = m.get("content")
        blocks = [{"type": "text", "text": c}] if isinstance(c, str) else (c or [])
        if m.get("role") == "assistant":
            out.append(msg("assistant", text_of([b for b in blocks if b.get("type") == "text"]),
                           sum(1 for b in blocks if b.get("type") == "tool_use")))
            continue
        for b in blocks:
            if b.get("type") == "tool_result":
                out.append(msg("tool", text_of(b.get("content"))))
        texts = [b for b in blocks if b.get("type") == "text"]
        imgs = ["data:%s;base64,%s" % (b["source"].get("media_type", ""), b["source"].get("data", ""))
                for b in blocks if b.get("type") == "image" and isinstance(b.get("source"), dict)]
        if texts or imgs:
            out.append(msg("user", text_of(texts), 0, imgs))
    return out


def conv_responses(body):
    out = []
    if body.get("instructions"):
        out.append(msg("system", body["instructions"]))
    items = body.get("input", [])
    if isinstance(items, str):
        items = [{"role": "user", "content": items}]
    for it in items:
        kind = it.get("type", "message")
        if kind == "function_call":
            out.append(msg("assistant", "", 1))
        elif kind == "function_call_output":
            o = it.get("output")
            out.append(msg("tool", o if isinstance(o, str) else text_of(o)))
        elif kind == "message":
            role = {"developer": "system"}.get(it.get("role"), it.get("role", "user"))
            c = it.get("content")
            imgs = [p.get("image_url", "") for p in (c if isinstance(c, list) else [])
                    if isinstance(p, dict) and p.get("type") == "input_image"]
            out.append(msg(role, text_of(c), 0, imgs))
    return out


def conv_gemini(body):
    out = []
    si = body.get("systemInstruction") or body.get("system_instruction")
    if si:
        out.append(msg("system", text_of(si.get("parts"))))
    for c in body.get("contents", []):
        parts = c.get("parts", [])
        texts = [p for p in parts if "text" in p and not p.get("thought")]
        if c.get("role") == "model":
            out.append(msg("assistant", text_of(texts), sum(1 for p in parts if "functionCall" in p)))
            continue
        for p in parts:
            r = p.get("functionResponse") or p.get("function_response")
            if r:
                resp = r.get("response", {})
                t = resp.get("output", resp.get("content", resp.get("result")))
                out.append(msg("tool", t if isinstance(t, str) else json.dumps(resp)))
        imgs = []
        for p in parts:
            d = p.get("inlineData") or p.get("inline_data")
            if d:
                imgs.append("data:%s;base64,%s" % (d.get("mimeType", d.get("mime_type", "")), d.get("data", "")))
        if texts or imgs:
            out.append(msg("user", text_of(texts), 0, imgs))
    return out


CONV = {"openai-chat": conv_openai_chat, "anthropic": conv_anthropic,
        "openai-responses": conv_responses, "gemini": conv_gemini}


def is_stt(path):
    """The speech-to-text endpoints (BISE-298): OpenAI's and Mistral's
    /audio/transcriptions, ElevenLabs' /speech-to-text, Deepgram's /listen."""
    p = path.split("?")[0].rstrip("/")
    return p.endswith("/audio/transcriptions") or p.endswith("/speech-to-text") or p.endswith("/listen")


def family_of(path):
    p = path.split("?")[0].rstrip("/")
    if p.endswith("/messages"):
        return "anthropic"
    if p.endswith("/responses"):
        return "openai-responses"
    if ":generateContent" in p or ":streamGenerateContent" in p:
        return "gemini"
    return "openai-chat"


def agent_of(conv):
    for m in conv:
        if m["role"] == "system":
            # the hub's one-shot call for a task's role line (BISE-126)
            if m["text"].startswith("# bise role line"):
                return "(role line)"
            # auto mode's checker as a chat model (approvals, design §4.2)
            if m["text"].startswith("# bise checker"):
                return "(checker)"
            g = re.search(r"# Your role: task `([^`]+)`", m["text"])
            if g:
                return g.group(1)
            if "# Your role: `main`" in m["text"]:
                return "main"
    return "?"


def last_user(conv):
    """index of the last real user message (the state block is not one)"""
    idx = None
    for i, m in enumerate(conv):
        if m["role"] == "user" and not m["text"].lstrip().startswith("<bise_state>"):
            idx = i
    return idx


# ------------------------------------------------------------------ script

def script_of(user):
    # the hub's notes about the past are not a script to run
    user = re.sub(r"<bise_notes>.*?</bise_notes>", "", user, flags=re.S)
    return re.sub(r"<task_status>.*?</task_status>", "", user, flags=re.S).strip()


def errors_of(user):
    out = []
    for kind, n, retry in ERROR.findall(user):
        out += [{"kind": kind, "retry": int(retry or 1)}] * int(n or 1)
    return out


def reply_for(conv, seen=0):
    """The turn for this conversation: {"text", "reasoning", "calls":
    [{"id", "name", "args"}], "error", "fixture"}. `seen`: how many
    requests for the same user message came before this one."""
    turn = {"text": "", "reasoning": "", "calls": [], "error": None, "fixture": None}
    if agent_of(conv) == "(role line)":
        # a fixed line (the tests read it in the snapshot), never a script
        turn["text"] = "Fake Role Line."
        return turn
    if agent_of(conv) == "(checker)":
        # strict JSON: contained unless the state names a force push or
        # an rm -rf (then a card), never secrets
        state = " ".join(m["text"] for m in conv if m["role"] == "user")
        risky = "--force" in state or "rm -rf" in state
        turn["text"] = json.dumps({"contained": not risky, "serves_task": True, "secrets": False})
        return turn
    idx = last_user(conv)
    if idx is None:
        turn["text"] = "ack: (nothing)"
        return turn
    user = script_of(conv[idx]["text"])
    errs = errors_of(user)
    if seen < len(errs):
        turn["error"] = errs[seen]
        return turn
    fx = FIXTURE.search(user)
    if fx and seen == len(errs):
        turn["fixture"] = fx.group(1)
        return turn
    think = THINK.search(user)
    turn["reasoning"] = think.group(1).strip() if think else ""
    after = conv[idx + 1:]
    calls_done = sum(m["calls"] for m in after if m["role"] == "assistant")
    marks = MARK.findall(user) or INNER.findall(user)
    if calls_done < len(marks):
        tool, arg = marks[calls_done]
        args = {"name": arg.strip()} if tool == "skill" else {"arg": arg.strip()}
        # `[[edit: JSON]]`, `[[write_file: JSON]]`: Vibe's edit tools
        # take JSON args (approvals-edit)
        if tool in ("edit", "write_file"):
            args = json.loads(arg, strict=False)
        # `[[bash: CMD @@ DESC]]`: the call carries a description (BISE-223)
        if tool == "bash" and " @@ " in arg:
            cmd, desc = arg.split(" @@ ", 1)
            args = {"arg": cmd.strip(), "description": desc.strip()}
        turn["calls"] = [{"id": "call_%d_%d" % (idx, calls_done), "name": tool, "args": args}]
        return turn
    results = [m["text"] for m in after if m["role"] == "tool"]
    if results:
        turn["text"] = "done: " + results[-1].strip()[:400]
        return turn
    # an image comes framed as text `<image name=[Image #1] path="…">`, the
    # image, text `</image>` (docs/images.md): a model says `[Image #1]`,
    # it does not echo the framing
    user = re.sub(r'<image name=(\[[^\]]*\])[^>]*>\s*</image>', r"\1", user)
    turn["text"] = "ack: " + " ".join(user.split())[:300]
    return turn


def pieces(s, n=3):
    """s in n pieces (at least one): the deltas a real stream sends"""
    if not s:
        return [""]
    k = max(1, -(-len(s) // n))
    return [s[i:i + k] for i in range(0, len(s), k)]


# ------------------------------------------------------------------ errors
# (status, headers, body) of an error reply, per family.

def error_reply(family, err):
    kind = err["kind"]
    if kind == "badname":
        # BISE-293: a final 400, the API's words for a tool name off its pattern
        return 400, {}, name_error(family, {"tools": [{"name": "self.compact", "function": {"name": "self.compact"}}]}) or {}
    status = {"429": 429, "500": 500, "stream": 500}.get(kind, 529 if family == "anthropic" else 503)
    hdr = {"retry-after": str(err.get("retry", 1))} if status in (429, 503, 529) else {}
    if family == "anthropic":
        t, m = {429: ("rate_limit_error", "Number of request tokens has exceeded your per-minute rate limit"),
                500: ("api_error", "Internal server error"),
                529: ("overloaded_error", "Overloaded")}[status]
        body = {"type": "error", "error": {"type": t, "message": m}, "request_id": "req_fake"}
    elif family == "gemini":
        s, m = {429: ("RESOURCE_EXHAUSTED", "Resource has been exhausted (e.g. check quota)."),
                500: ("INTERNAL", "An internal error has occurred."),
                503: ("UNAVAILABLE", "The model is overloaded. Please try again later.")}[status]
        body = {"error": {"code": status, "message": m, "status": s}}
    else:
        t, c, m = {429: ("requests", "rate_limit_exceeded", "Rate limit reached for requests"),
                   500: ("server_error", None, "The server had an error while processing your request. Sorry about that!"),
                   503: ("server_error", None, "The engine is currently overloaded, please try again later")}[status]
        body = {"error": {"message": m, "type": t, "param": None, "code": c}}
    return status, hdr, body


# --------------------------------------------------------------- renderers
# A turn as the whole JSON reply (whole_*) or as SSE events (sse_*: a list
# of (event name or None, data dict or "[DONE]")). `mid_error`: the stream
# breaks with the family's error event after its start.

def whole_openai_chat(turn, model):
    m = {"role": "assistant", "content": turn["text"]}
    if turn["reasoning"]:
        m["reasoning_content"] = turn["reasoning"]
    if turn["calls"]:
        m["tool_calls"] = [{"id": c["id"], "type": "function",
                            "function": {"name": c["name"], "arguments": json.dumps(c["args"])}}
                           for c in turn["calls"]]
    return {"id": "fake", "object": "chat.completion", "created": 0, "model": model,
            "choices": [{"index": 0, "message": m,
                         "finish_reason": "tool_calls" if turn["calls"] else "stop"}],
            "usage": {"prompt_tokens": USAGE["input"], "completion_tokens": USAGE["output"],
                      "total_tokens": USAGE["input"] + USAGE["output"],
                      "prompt_tokens_details": {"cached_tokens": USAGE["cached"]},
                      "completion_tokens_details": {"reasoning_tokens": USAGE["reasoning"]}}}


def sse_openai_chat(turn, model, include_usage=False, mid_error=False):
    base = {"id": "chatcmpl-fake", "object": "chat.completion.chunk", "created": 0, "model": model,
            "system_fingerprint": "fp_fake"}

    def chunk(delta, finish=None):
        d = dict(base, choices=[{"index": 0, "delta": delta, "logprobs": None, "finish_reason": finish}])
        if include_usage:
            d["usage"] = None
        return (None, d)
    ev = [chunk({"role": "assistant", "content": "", "refusal": None})]
    if mid_error:
        # OpenRouter's documented mid-stream error: a chunk with "error"
        # and finish_reason "error" (openai.com sends no event for it)
        ev.append((None, dict(base, error={"code": "server_error", "message": "Internal server error"},
                              choices=[{"index": 0, "delta": {"content": ""}, "finish_reason": "error"}])))
        return ev
    for p in pieces(turn["reasoning"]) if turn["reasoning"] else []:
        ev.append(chunk({"content": None, "reasoning_content": p}))
    for p in pieces(turn["text"]) if turn["text"] else []:
        ev.append(chunk({"content": p}))
    for i, c in enumerate(turn["calls"]):
        ev.append(chunk({"tool_calls": [{"index": i, "id": c["id"], "type": "function",
                                         "function": {"name": c["name"], "arguments": ""}}]}))
        for p in pieces(json.dumps(c["args"])):
            ev.append(chunk({"tool_calls": [{"index": i, "function": {"arguments": p}}]}))
    ev.append(chunk({}, "tool_calls" if turn["calls"] else "stop"))
    if include_usage:
        u = whole_openai_chat(turn, model)["usage"]
        ev.append((None, dict(base, choices=[], usage=u)))
    ev.append((None, "[DONE]"))
    return ev


def anth_blocks(turn):
    out = []
    if turn["reasoning"]:
        out.append({"type": "thinking", "thinking": turn["reasoning"], "signature": "fake-sig"})
    if turn["text"] or not turn["calls"]:
        out.append({"type": "text", "text": turn["text"]})
    for c in turn["calls"]:
        out.append({"type": "tool_use", "id": "toolu_" + c["id"], "name": c["name"], "input": c["args"]})
    return out


def anth_usage(output):
    return {"input_tokens": USAGE["input"] - USAGE["cached"], "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": USAGE["cached"], "output_tokens": output}


def whole_anthropic(turn, model):
    return {"id": "msg_fake", "type": "message", "role": "assistant", "model": model,
            "content": anth_blocks(turn), "stop_reason": "tool_use" if turn["calls"] else "end_turn",
            "stop_sequence": None, "usage": anth_usage(USAGE["output"])}


def sse_anthropic(turn, model, mid_error=False):
    start = dict(whole_anthropic(turn, model), content=[], stop_reason=None, usage=anth_usage(1))
    ev = [("message_start", {"type": "message_start", "message": start}), ("ping", {"type": "ping"})]
    if mid_error:
        ev.append(("error", {"type": "error", "error": {"type": "overloaded_error", "message": "Overloaded"}}))
        return ev
    for i, b in enumerate(anth_blocks(turn)):
        if b["type"] == "thinking":
            first = {"type": "thinking", "thinking": ""}
            deltas = [{"type": "thinking_delta", "thinking": p} for p in pieces(b["thinking"])]
            deltas.append({"type": "signature_delta", "signature": b["signature"]})
        elif b["type"] == "text":
            first = {"type": "text", "text": ""}
            deltas = [{"type": "text_delta", "text": p} for p in pieces(b["text"])]
        else:
            first = dict(b, input={})
            deltas = [{"type": "input_json_delta", "partial_json": p}
                      for p in [""] + pieces(json.dumps(b["input"]))]
        ev.append(("content_block_start", {"type": "content_block_start", "index": i, "content_block": first}))
        for d in deltas:
            ev.append(("content_block_delta", {"type": "content_block_delta", "index": i, "delta": d}))
        ev.append(("content_block_stop", {"type": "content_block_stop", "index": i}))
    ev.append(("message_delta", {"type": "message_delta",
                                 "delta": {"stop_reason": "tool_use" if turn["calls"] else "end_turn",
                                           "stop_sequence": None},
                                 "usage": {"output_tokens": USAGE["output"]}}))
    ev.append(("message_stop", {"type": "message_stop"}))
    return ev


def resp_items(turn, encrypted):
    out = []
    if turn["reasoning"]:
        it = {"id": "rs_fake", "type": "reasoning",
              "summary": [{"type": "summary_text", "text": turn["reasoning"]}]}
        if encrypted:
            it["encrypted_content"] = "fake-enc"
        out.append(it)
    if turn["text"] or not turn["calls"]:
        out.append({"id": "msg_fake", "type": "message", "status": "completed", "role": "assistant",
                    "content": [{"type": "output_text", "text": turn["text"], "annotations": []}]})
    for c in turn["calls"]:
        out.append({"id": "fc_" + c["id"], "type": "function_call", "status": "completed",
                    "call_id": c["id"], "name": c["name"], "arguments": json.dumps(c["args"])})
    return out


def whole_responses(turn, model, encrypted=False, status="completed"):
    return {"id": "resp_fake", "object": "response", "created_at": 0, "status": status,
            "error": None, "incomplete_details": None, "model": model,
            "output": resp_items(turn, encrypted) if status == "completed" else [],
            "usage": {"input_tokens": USAGE["input"], "input_tokens_details": {"cached_tokens": USAGE["cached"]},
                      "output_tokens": USAGE["output"],
                      "output_tokens_details": {"reasoning_tokens": USAGE["reasoning"]},
                      "total_tokens": USAGE["input"] + USAGE["output"]} if status == "completed" else None}


def sse_responses(turn, model, encrypted=False, mid_error=False):
    ev = []

    def add(t, **d):
        ev.append((t, dict({"type": t, "sequence_number": len(ev)}, **d)))
    add("response.created", response=whole_responses(turn, model, status="in_progress"))
    add("response.in_progress", response=whole_responses(turn, model, status="in_progress"))
    if mid_error:
        add("error", code="server_error", message="The server had an error while processing your request.",
            param=None)
        return ev
    for oi, it in enumerate(resp_items(turn, encrypted)):
        iid = it["id"]
        if it["type"] == "reasoning":
            add("response.output_item.added", output_index=oi, item=dict(it, summary=[]))
            text = it["summary"][0]["text"]
            add("response.reasoning_summary_part.added", item_id=iid, output_index=oi, summary_index=0,
                part={"type": "summary_text", "text": ""})
            for p in pieces(text):
                add("response.reasoning_summary_text.delta", item_id=iid, output_index=oi, summary_index=0,
                    delta=p)
            add("response.reasoning_summary_text.done", item_id=iid, output_index=oi, summary_index=0, text=text)
            add("response.reasoning_summary_part.done", item_id=iid, output_index=oi, summary_index=0,
                part={"type": "summary_text", "text": text})
        elif it["type"] == "message":
            add("response.output_item.added", output_index=oi,
                item=dict(it, status="in_progress", content=[]))
            text = it["content"][0]["text"]
            add("response.content_part.added", item_id=iid, output_index=oi, content_index=0,
                part={"type": "output_text", "text": "", "annotations": []})
            for p in pieces(text):
                add("response.output_text.delta", item_id=iid, output_index=oi, content_index=0, delta=p,
                    logprobs=[])
            add("response.output_text.done", item_id=iid, output_index=oi, content_index=0, text=text,
                logprobs=[])
            add("response.content_part.done", item_id=iid, output_index=oi, content_index=0,
                part=it["content"][0])
        else:
            add("response.output_item.added", output_index=oi,
                item=dict(it, status="in_progress", arguments=""))
            for p in pieces(it["arguments"]):
                add("response.function_call_arguments.delta", item_id=iid, output_index=oi, delta=p)
            add("response.function_call_arguments.done", item_id=iid, output_index=oi,
                arguments=it["arguments"])
        add("response.output_item.done", output_index=oi, item=it)
    add("response.completed", response=whole_responses(turn, model, encrypted))
    return ev


def gem_usage(final):
    u = {"promptTokenCount": USAGE["input"], "totalTokenCount": USAGE["input"]}
    if final:
        u.update(candidatesTokenCount=USAGE["output"], thoughtsTokenCount=USAGE["reasoning"],
                 cachedContentTokenCount=USAGE["cached"],
                 totalTokenCount=USAGE["input"] + USAGE["output"] + USAGE["reasoning"])
    return u


def gem_chunk(parts, model, finish=None):
    cand = {"content": {"parts": parts, "role": "model"}, "index": 0}
    if finish:
        cand["finishReason"] = finish
    return {"candidates": [cand], "usageMetadata": gem_usage(bool(finish)),
            "modelVersion": model, "responseId": "fake-response"}


def gem_parts(turn):
    """the parts in stream order: thoughts, text, then whole function calls
    (Gemini never splits a functionCall); the first functionCall of a step
    carries the thoughtSignature (Gemini 3)"""
    out = [[{"text": p, "thought": True}] for p in pieces(turn["reasoning"])] if turn["reasoning"] else []
    out += [[{"text": p}] for p in pieces(turn["text"])] if turn["text"] else []
    calls = [{"functionCall": {"name": c["name"], "args": c["args"]}} for c in turn["calls"]]
    if calls:
        calls[0]["thoughtSignature"] = "fake-thought-sig"
        out.append(calls)
    return out or [[{"text": ""}]]


def whole_gemini(turn, model):
    return gem_chunk([p for ps in gem_parts(turn) for p in ps], model, "STOP")


def sse_gemini(turn, model, mid_error=False):
    ps = gem_parts(turn)
    if mid_error:
        return [(None, gem_chunk(ps[0], model)),
                (None, {"error": {"code": 503, "message": "The model is overloaded. Please try again later.",
                                  "status": "UNAVAILABLE"}})]
    return [(None, gem_chunk(p, model, "STOP" if i == len(ps) - 1 else None)) for i, p in enumerate(ps)]


def render(family, turn, model="fake", stream=True, body=None, mid_error=False):
    """the events (stream) or the JSON body (whole) for a turn"""
    body = body or {}
    enc = "reasoning.encrypted_content" in (body.get("include") or [])
    if not stream:
        return {"openai-chat": whole_openai_chat, "anthropic": whole_anthropic,
                "gemini": whole_gemini}.get(family, lambda t, m: whole_responses(t, m, enc))(turn, model)
    if family == "openai-chat":
        inc = bool((body.get("stream_options") or {}).get("include_usage"))
        return sse_openai_chat(turn, model, inc, mid_error)
    if family == "anthropic":
        return sse_anthropic(turn, model, mid_error)
    if family == "openai-responses":
        return sse_responses(turn, model, enc, mid_error)
    return sse_gemini(turn, model, mid_error)


def sse_bytes(family, events):
    # Gemini ends its events with CRLF CRLF (captured from the live API);
    # the others with LF LF. Named events for Anthropic and Responses.
    sep = "\r\n\r\n" if family == "gemini" else "\n\n"
    out = []
    for name, data in events:
        d = data if isinstance(data, str) else json.dumps(data, separators=(",", ":"))
        out.append(("event: %s\n" % name if name else "") + "data: " + d + sep)
    return "".join(out).encode()


def openai_view(turn):
    """the turn as the old log's {content, tool_calls} (tui_queue reads it)"""
    if turn["calls"]:
        return {"content": "", "tool_calls": [
            {"id": c["id"], "type": "function",
             "function": {"name": c["name"], "arguments": json.dumps(c["args"])}} for c in turn["calls"]]}
    return {"content": turn["text"]}


# ------------------------------------------------------------------ server

class State:
    lock = threading.Lock()
    seen = {}  # (family, agent, user message) -> requests so far


def fixture_reply(family, name, stream):
    d = os.path.join(FIXTURES, family)
    want = [name + ".sse"] if stream else []
    want.append(name + ".json")
    for f in want:
        p = os.path.join(d, f)
        if os.path.exists(p):
            return 200, open(p, "rb").read(), f.endswith(".sse")
    for f in sorted(os.listdir(d)) if os.path.isdir(d) else []:
        g = re.fullmatch(re.escape(name) + r"\.(\d{3})\.json", f)
        if g:
            return int(g.group(1)), open(os.path.join(d, f), "rb").read(), False
    return 404, json.dumps({"error": "no fixture %s/%s" % (family, name)}).encode(), False


class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):
        pass

    def send(self, status, data, ctype="application/json", headers=None):
        self.send_response(status)
        self.send_header("content-type", ctype)
        self.send_header("content-length", str(len(data)))
        self.send_header("connection", "close")
        for k, v in (headers or {}).items():
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(data)
        self.close_connection = True

    def send_sse(self, data_chunks):
        # chunked, as the real APIs do over HTTP/1.1
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.send_header("cache-control", "no-cache")
        self.send_header("transfer-encoding", "chunked")
        self.send_header("connection", "close")
        self.end_headers()
        for c in data_chunks:
            if c:
                self.wfile.write(b"%x\r\n%s\r\n" % (len(c), c))
                self.wfile.flush()
        self.wfile.write(b"0\r\n\r\n")
        self.close_connection = True

    def stt(self, raw):
        """BISE-298: speech to text (a recording, the voice key check).
        The key's word picks the answer: "bad" 401, "broke" 402 (until
        $FAKE_CREDIT exists), "down" 503; else {"text": $FAKE_STT_TEXT}
        ("" by default: the check's silence)."""
        auth = " ".join(self.headers.get(h, "") for h in ("authorization", "x-api-key", "xi-api-key"))
        credit = os.environ.get("FAKE_CREDIT")
        broke = "broke" in auth and not (credit and os.path.exists(credit))
        m = re.search(rb'name="model(?:_id)?"\r\n\r\n([^\r]*)', raw)
        model = m.group(1).decode() if m else ""
        with open(LOG, "a") as f:
            f.write(json.dumps({"agent": "", "family": "stt", "path": self.path, "model": model,
                                "bytes": len(raw)}) + "\n")
        if "bad" in auth:
            self.send(401, json.dumps({"detail": "Invalid API Key"}).encode())
        elif broke:
            self.send(402, json.dumps({"error": {"type": "billing_error", "message": "insufficient credit balance"}}).encode())
        elif "down" in auth:
            self.send(503, json.dumps({"error": {"message": "the service is overloaded"}}).encode())
        else:
            self.send(200, json.dumps({"model": model, "text": os.environ.get("FAKE_STT_TEXT", "")}).encode())

    def do_POST(self):
        n = int(self.headers.get("content-length", "0"))
        raw = self.rfile.read(n)
        if is_stt(self.path):
            return self.stt(raw)
        body = json.loads(raw or b"{}")
        family = family_of(self.path)
        stream = (":streamGenerateContent" in self.path) if family == "gemini" else body.get("stream") is True
        sse = stream and (family != "gemini" or "alt=sse" in self.path)
        # BISE-266: a key holding "bad" is refused (401), one holding
        # "broke" has no credit (402): the first run's key check
        auth = " ".join(self.headers.get(h, "") for h in ("authorization", "x-api-key", "x-goog-api-key"))
        # $FAKE_CREDIT: once that file exists, the "broke" account has
        # credit (the user added some: tui_stuck_start_tmux.py)
        credit = os.environ.get("FAKE_CREDIT")
        broke = "broke" in auth and not (credit and os.path.exists(credit))
        if "bad" in auth or broke:
            bad = "bad" in auth
            said = "invalid api key" if bad else "insufficient credit balance"
            if "broke-url" in auth:
                # BISE-287: OpenAI's words, its billing page then `."`
                said = ("You have no credits remaining. Add credits to continue using the API at "
                        "https://platform.openai.com/settings/organization/billing/.\"")
            err = {"error": {"type": "authentication_error" if bad else "billing_error", "message": said}}
            self.send(401 if bad else 402, json.dumps(err).encode())
            return
        # BISE-293: the real APIs refuse a tool name off their pattern
        # (OpenAI refused "self.compact" on the user's first message)
        bad_name = name_error(family, body) or request_error(family, body)
        if bad_name:
            with open(LOG, "a") as f:
                f.write(json.dumps({"agent": "", "family": family, "path": self.path,
                                    "status": 400, "name_error": bad_name}) + "\n")
            self.send(400, json.dumps(bad_name).encode())
            return
        conv = CONV[family](body)
        agent = agent_of(conv)
        idx = last_user(conv)
        key = (family, agent, conv[idx]["text"] if idx is not None else "")
        with State.lock:
            seen = State.seen.get(key, 0)
            State.seen[key] = seen + 1
        turn = reply_for(conv, seen)
        model = body.get("model") or self.path.split("/models/")[-1].split(":")[0] or "fake"
        status = 200
        if turn["fixture"]:
            status, data, is_sse = fixture_reply(family, turn["fixture"], sse)
            if is_sse:
                self.send_sse([data])
            else:
                self.send(status, data)
        elif turn["error"] and not (turn["error"]["kind"] == "stream" and stream):
            status, hdr, err = error_reply(family, turn["error"])
            self.send(status, json.dumps(err).encode(), headers=hdr)
        else:
            mid = bool(turn["error"])
            out = render(family, turn, model, stream, body, mid)
            if not stream:
                self.send(200, json.dumps(out).encode())
            elif sse:
                self.send_sse([sse_bytes(family, [e]) for e in out])
            else:  # Gemini without alt=sse: one JSON array
                self.send(200, json.dumps([d for _, d in out]).encode())
        u = conv[idx] if idx is not None else msg("user")
        last = [m for m in conv if m["role"] == "user"]
        with open(LOG, "a") as f:
            f.write(json.dumps({"agent": agent, "last_user": (last[-1]["text"] if last else "")[:3000],
                                "user": u["text"][:3000], "reply": openai_view(turn),
                                # BISE-240: a long user text past the 3000 kept above
                                "user_len": len(u["text"]), "user_tail": u["text"][-300:],
                                # every user text of the conversation (a resumed session keeps its history)
                                "users": [m["text"][:200] for m in last],
                                "images": [i[:200] for m in conv for i in m["images"]],
                                # each image whole, as a hash (a body spliced at
                                # write time must carry every byte)
                                "image_sha": [hashlib.sha256(i.encode()).hexdigest() for m in conv for i in m["images"]],
                                "family": family, "path": self.path, "stream": stream, "status": status,
                                "error": turn["error"], "fixture": turn["fixture"],
                                # BISE-135: the model and effort the call asked for
                                "model": model, "effort": effort_of(body),
                                # BISE-232: the AGENTS.md block of the system prompt
                                "agents_md": agents_md_of(conv),
                                # approvals-edit: the request's tools, each
                                # with its description, and the system text
                                "tools": tool_list(family, body),
                                # image-tag: where each Anthropic image block
                                # sits (a tool_result's or a message's), and
                                # the texts that stand for an image not sent
                                "anth_images": anth_image_where(body) if family == "anthropic" else [],
                                "unavailable": re.findall(r"\[image unavailable: .*?\)\]", json.dumps(body)),
                                "tool_texts": [m["text"] for m in conv if m["role"] == "tool"],
                                "system": "\n".join(m["text"] for m in conv if m["role"] == "system"),
                                # a gateway: the request's headers (catalog headers_env, key_command)
                                "headers": {k.lower(): v for k, v in self.headers.items()}}) + "\n")


# ---------------------------------------------------------------- tool names
# BISE-293: each API's tool-name pattern, enforced like the real ones
# (the strictest, OpenAI's, is ^[a-zA-Z0-9_-]+$ with 64 chars at most):
# a tool of the request or a call of its history off the pattern gets
# the API's own 400.
NAME_OK = re.compile(r"^[a-zA-Z0-9_-]{1,64}$")
ANTH_NAME_OK = re.compile(r"^[a-zA-Z0-9_-]{1,128}$")


def tool_list(family, body):
    """[name, description] of the request's tools, in order"""
    out = []
    for t in body.get("tools") or []:
        if family == "openai-chat":
            f = t.get("function") or {}
            out.append([f.get("name", ""), f.get("description", "")])
        elif family == "gemini":
            out += [[d.get("name", ""), d.get("description", "")] for d in t.get("functionDeclarations") or []]
        else:
            out.append([t.get("name", ""), t.get("description", "")])
    return out


def tool_names(family, body):
    """(where, name) of every tool name a request carries: its tools and
    its history's calls"""
    out = []
    for i, t in enumerate(body.get("tools") or []):
        if family == "openai-chat":
            out.append(("tools[%d].function.name" % i, (t.get("function") or {}).get("name", "")))
        elif family == "gemini":
            for j, d in enumerate(t.get("functionDeclarations") or []):
                out.append(("tools[%d].functionDeclarations[%d].name" % (i, j), d.get("name", "")))
        else:
            out.append(("tools.%d.name" % i if family == "anthropic" else "tools[%d].name" % i, t.get("name", "")))
    inp = body.get("input")
    for i, it in enumerate(inp if isinstance(inp, list) else []):
        if isinstance(it, dict) and it.get("type") == "function_call":
            out.append(("input[%d].name" % i, it.get("name", "")))
    for i, m in enumerate(body.get("messages") or []):
        for c in m.get("tool_calls") or []:
            out.append(("messages[%d].tool_calls.function.name" % i, (c.get("function") or {}).get("name", "")))
        if isinstance(m.get("content"), list):
            for b in m["content"]:
                if isinstance(b, dict) and b.get("type") == "tool_use":
                    out.append(("messages.%d.content.tool_use.name" % i, b.get("name", "")))
    return out


def oai_error(where, message, code="invalid_value"):
    return {"error": {"message": message, "type": "invalid_request_error", "param": where, "code": code}}


def b64_ok(data):
    try:
        return isinstance(data, str) and data != "" and len(base64.b64decode(data, validate=True)) > 0
    except (ValueError, TypeError):
        return False


def anth_image_error(body):
    """the Anthropic 400 for the first image block with bad base64, None
    when every image is fine (messages.N.content.M[.tool_result.content.K])"""
    def bad(b, where):
        if isinstance(b, dict) and b.get("type") == "image" and not b64_ok((b.get("source") or {}).get("data")):
            return {"type": "error", "error": {"type": "invalid_request_error",
                                               "message": where + ".image.source.base64: invalid base64 data"}}
        return None
    for n, m in enumerate(body.get("messages", [])):
        c = m.get("content")
        for k, b in enumerate(c if isinstance(c, list) else []):
            e = bad(b, "messages.%d.content.%d" % (n, k))
            if e:
                return e
            inner = b.get("content") if isinstance(b, dict) and b.get("type") == "tool_result" else None
            for j, ib in enumerate(inner if isinstance(inner, list) else []):
                e = bad(ib, "messages.%d.content.%d.tool_result.content.%d" % (n, k, j))
                if e:
                    return e
    return None


def anth_image_where(body):
    """'message' or 'tool_result' for each image block, in order"""
    out = []
    for m in body.get("messages", []):
        c = m.get("content")
        for b in c if isinstance(c, list) else []:
            if isinstance(b, dict) and b.get("type") == "image":
                out.append("message")
            inner = b.get("content") if isinstance(b, dict) and b.get("type") == "tool_result" else None
            out += ["tool_result" for ib in (inner if isinstance(inner, list) else [])
                    if isinstance(ib, dict) and ib.get("type") == "image"]
    return out


def request_error(family, body):
    """BISE-147: what the real OpenAI APIs refuse beyond the names, None
    when the request is fine. Chat Completions: function tools with
    reasoning_effort on a gpt-6 model. Responses (store false): a
    reasoning item without its encrypted_content or with an id (OpenAI
    looks the id up and finds nothing stored), a function_call_output
    whose call_id no function_call before it has, a function tool
    without a name. Anthropic (2026-10-02, three agents stuck): an image
    block whose data is not base64, in a message or a tool_result, gets
    the real API's 400 with its path."""
    if family == "anthropic":
        return anth_image_error(body)
    if family == "openai-chat":
        if str(body.get("model", "")).startswith("gpt-6") and body.get("tools") and body.get("reasoning_effort") \
                and body.get("reasoning_effort") != "none":
            return oai_error("reasoning_effort", "Function tools with reasoning_effort are not supported for %s in "
                             "/v1/chat/completions. To use function tools, use /v1/responses or set "
                             "reasoning_effort to 'none'." % body["model"], "unsupported_parameter")
        return None
    if family != "openai-responses":
        return None
    inp = body.get("input")
    calls = set()
    for i, it in enumerate(inp if isinstance(inp, list) else []):
        kind = it.get("type", "message")
        if kind == "reasoning":
            if it.get("id") and body.get("store") is False:
                return oai_error("input[%d].id" % i, "Item with id '%s' not found. Items are not persisted when "
                                 "`store` is set to false." % it["id"], "item_not_found")
            if not it.get("encrypted_content"):
                return oai_error("input[%d].encrypted_content" % i, "Reasoning items without encrypted_content "
                                 "can not be replayed when `store` is set to false.")
        elif kind == "function_call":
            calls.add(it.get("call_id"))
        elif kind == "function_call_output" and it.get("call_id") not in calls:
            return oai_error("input", "No tool call found for function call output with call_id %s." %
                             it.get("call_id"))
    return None


def name_error(family, body):
    """the API's 400 body for the first tool name off its pattern, None
    when every name is fine"""
    ok = ANTH_NAME_OK if family == "anthropic" else NAME_OK
    for where, name in tool_names(family, body):
        if not ok.match(name or ""):
            if family == "anthropic":
                return {"type": "error", "error": {"type": "invalid_request_error",
                        "message": "%s: String should match pattern '^[a-zA-Z0-9_-]{1,128}$'" % where}}
            return {"error": {"message": "Invalid '%s': string does not match pattern. Expected a string "
                              "that matches the pattern '^[a-zA-Z0-9_-]+$'." % where,
                              "type": "invalid_request_error", "param": where, "code": "invalid_value"}}
    return None


def effort_of(body):
    """the reasoning effort a request asks for (BISE-135): Chat's
    reasoning_effort, Anthropic's output_config.effort or thinking
    budget, "" when none"""
    if body.get("reasoning_effort"):
        return body["reasoning_effort"]
    oc = body.get("output_config") or {}
    if oc.get("effort"):
        return oc["effort"]
    th = body.get("thinking") or {}
    if th.get("budget_tokens"):
        return "budget:%d" % th["budget_tokens"]
    return ""


class Server(http.server.ThreadingHTTPServer):
    """HTTPServer.server_bind asks socket.getfqdn("127.0.0.1"): a reverse
    DNS lookup that hangs 10 s and more on the GitHub macOS runners, so
    PORT came too late for test-install.sh (release v2026.9.30). The
    name only fills SERVER_NAME for CGI: skip the lookup."""
    daemon_threads = True

    def server_bind(self):
        socketserver.TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address[:2]


def serve(port=0):
    """a server in this process (a thread): (server, port)"""
    srv = Server(("127.0.0.1", port), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, srv.server_address[1]


def bend_literal(text):
    """a Bend string literal (LAWS fixtures): escapes like LAWS.bend's"""
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"').replace("\r", "\\r").replace("\n", "\\n") + '"'


# the fixtures this server writes itself (`fake_provider.py fixtures`):
# the shapes above, as files a family's laws can start from before a
# recorded reply exists. provider_families.py fails when one is stale.
FAKE_TURN = {"text": "I'll list /tmp.", "reasoning": "The user wants a listing.",
             "calls": [{"id": "call_1_0", "name": "bash", "args": {"arg": "ls -la /tmp | head"}},
                       {"id": "call_1_1", "name": "bash", "args": {"arg": "pwd"}}],
             "error": None, "fixture": None}
FAKE_BODY = {"stream_options": {"include_usage": True}, "include": ["reasoning.encrypted_content"]}


def fake_fixtures(family):
    """{file name: (bytes, index entry)}"""
    src = "fake_provider.py fixtures (the shape of the docs in providers.md §8, not a recorded reply)"
    t = FAKE_TURN
    exp = {"text": t["text"], "reasoning": t["reasoning"],
           "calls": [{"name": c["name"], "args": c["args"]} for c in t["calls"]]}
    out = {
        "fake-tool-call.sse": (sse_bytes(family, render(family, t, "fake-model", True, FAKE_BODY)),
                               {"source": src, "expect": exp}),
        "fake-tool-call.json": ((json.dumps(render(family, t, "fake-model", False, FAKE_BODY), indent=1) + "\n")
                                .encode(), {"source": src, "expect": exp}),
    }
    ev = render(family, t, "fake-model", True, FAKE_BODY, mid_error=True)
    out["fake-stream-error.sse"] = (sse_bytes(family, ev), {"source": src, "expect": {"error": True}})
    for name, kind in (("fake-rate-limit", "429"), ("fake-overloaded", "overloaded"), ("fake-server-error", "500")):
        st, _, body = error_reply(family, {"kind": kind})
        out["%s.%d.json" % (name, st)] = ((json.dumps(body) + "\n").encode(),
                                          {"source": src, "expect": {"error": True}})
    return out


def write_fixtures():
    for fam in FAMILIES:
        d = os.path.join(FIXTURES, fam)
        os.makedirs(d, exist_ok=True)
        ip = os.path.join(d, "index.json")
        index = json.load(open(ip)) if os.path.exists(ip) else {}
        for name, (data, entry) in fake_fixtures(fam).items():
            open(os.path.join(d, name), "wb").write(data)
            index[name] = entry
        open(ip, "w").write(json.dumps(dict(sorted(index.items())), indent=1, ensure_ascii=False) + "\n")


def main(argv):
    if argv[:1] == ["fixtures"]:
        write_fixtures()
        return
    if argv[:1] == ["render"]:
        whole = "--whole" in argv
        fam, path = [a for a in argv[1:] if a != "--whole"]
        t = json.load(open(path))
        turn = {"text": t.get("text", ""), "reasoning": t.get("reasoning", ""), "error": None, "fixture": None,
                "calls": [{"id": "call_%d" % i, "name": c["name"], "args": c["args"]}
                          for i, c in enumerate(t.get("calls", []))]}
        if t.get("error"):
            st, _, err = error_reply(fam, {"kind": t["error"]})
            if t["error"] != "stream" or whole:
                sys.stdout.write(json.dumps(err) + "\n")
                return
        out = render(fam, turn, t.get("model", "fake"), not whole, t.get("body"), bool(t.get("error")))
        sys.stdout.write(json.dumps(out, indent=1) + "\n" if whole else sse_bytes(fam, out).decode())
        return
    if argv[:1] == ["bend"]:
        sys.stdout.write(bend_literal(open(argv[1]).read()) + "\n")
        return
    port = int(argv[0]) if argv else 0
    srv = Server(("127.0.0.1", port), H)
    print("PORT", srv.server_address[1], flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main(sys.argv[1:])
