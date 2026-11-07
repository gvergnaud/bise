#!/usr/bin/env python3
"""The model calls' times and the turns lost, read from the session logs
(c2-turn-survives d; architecture chantier 8 sizes its head timeout on
ttfb). Read-only: it opens <bise home>/sessions/*/events.jsonl and
writes nothing.

  scripts/call_times.py [--since 2026-11-08] [--home ~/.bise]

Per provider (the model id's part before "/"), from the usage events
that carry the times (build_ms, save_ms, ttfb_ms, total_ms, attempt;
older events have none): the count and p50 / p99 / p99.9 of each, and
how many calls needed more than one attempt. Then the timeouts by the
limit that ran out (request_failed's `limit`, "head" or "gap", and its
`waited_ms`; c8-gateway-hang): the time lost to hung heads. Then the
turns: how many
ended, how many failed or crashed (the lost ones), and the lost turns
by their first words.
"""
import argparse, glob, json, os
from collections import Counter, defaultdict

FIELDS = ("ttfb_ms", "total_ms", "build_ms", "save_ms")


def pct(xs, p):
    if not xs:
        return None
    xs = sorted(xs)
    i = min(len(xs) - 1, max(0, int(round(p / 100.0 * (len(xs) - 1)))))
    return xs[i]


def events(home, since):
    for path in glob.glob(os.path.join(home, "sessions", "*", "events.jsonl")):
        try:
            f = open(path, errors="replace")
        except OSError:
            continue
        with f:
            for line in f:
                try:
                    r = json.loads(line)
                except ValueError:
                    continue
                if since and r.get("at", "") < since:
                    continue
                yield path, r


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--since", default="", help="ISO date: only events from then on")
    ap.add_argument("--home", default=os.environ.get("BISE_HOME") or os.path.expanduser("~/.bise"))
    a = ap.parse_args()
    times = defaultdict(lambda: defaultdict(list))
    retried = Counter()
    ended, lost, why = 0, 0, Counter()
    failed_last = {}
    hung, hung_ms = Counter(), Counter()
    for path, r in events(a.home, a.since):
        t, d = r.get("type"), r.get("data") or {}
        if t == "usage" and "ttfb_ms" in d:
            prov = (d.get("model") or "?").split("/")[0]
            for k in FIELDS:
                if k == "ttfb_ms" and not d.get(k):
                    continue  # no response head: no ttfb
                times[prov][k].append(d.get(k) or 0)
            if (d.get("attempt") or 1) > 1:
                retried[prov] += 1
        elif t == "request_failed":
            failed_last[path] = ((d.get("error") or {}).get("message") or "")
            if d.get("limit"):
                hung[d["limit"]] += 1
                hung_ms[d["limit"]] += d.get("waited_ms") or 0
        elif t == "interrupted" and d.get("by") == "restart":
            failed_last[path] = "interrupted by a restart during " + str(d.get("during"))
        elif t == "turn_ended":
            ended += 1
            if d.get("outcome") in ("failed", "crashed"):
                lost += 1
                msg = d.get("error") or d.get("reason") or failed_last.get(path) or d.get("outcome")
                if isinstance(msg, dict):
                    msg = msg.get("message") or msg.get("kind")
                why[" ".join(str(msg).split()[:6])] += 1
    for prov in sorted(times):
        n = len(times[prov]["total_ms"])
        print("%s: %d calls with times, %d needed more than one attempt" % (prov, n, retried[prov]))
        for k in FIELDS:
            xs = times[prov][k]
            print("  %-9s n=%-6d p50=%-8s p99=%-8s p99.9=%s"
                  % (k, len(xs), pct(xs, 50), pct(xs, 99), pct(xs, 99.9)))
    if not times:
        print("no usage event with times yet (they start with c2-turn-survives d)")
    if hung:
        print("timeouts (c8-gateway-hang: the request_failed's limit, typed):")
        for lim in sorted(hung):
            print("  %-4s %5d retries, %.2f h waited" % (lim, hung[lim], hung_ms[lim] / 3600000.0))
    else:
        print("no timeout with its limit yet (it starts with c8-gateway-hang)")
    share = 100.0 * lost / ended if ended else 0.0
    print("turns: %d ended, %d lost (failed or crashed): %.2f %%" % (ended, lost, share))
    for w, n in why.most_common(12):
        print("  %4d  %s" % (n, w))


if __name__ == "__main__":
    main()
