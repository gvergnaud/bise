#!/usr/bin/env python3
"""How long the hub's loop was held (chantier c4-loop-never-waits): the
share of the hub's up time its loop sat in one message, and every block
over 5 s outside a start, from a hub.log.

  python3 scripts/hub_loop_held.py <hub.log> [--since 7d] [--json]

Read only. The lines it reads (daemon/loop_watch.rs writes them):
  <ms> hub start pid=...                       a run starts (its up time too)
  <ms> boot: boot done ...                     the run's start is over
  <ms> hub loop held 2.1 s on <msg> (<split>)  a hold of 1 s or more, at its end
  <ms> hub loop busy 30 s on <msg> (<split>)   a long hold, while it lasts
  <ms> hub loop free after 261 s on <msg> ...  a long hold, at its end
A run's up time: its start to its last line. A hold is counted at its
end; a run that ends while busy counts its last busy line. A hold shorter
than 1 s is in no line: the share is a floor.
"""
import argparse
import json
import re
import sys

START = re.compile(r"^(\d+) hub start pid=")
BOOTED = re.compile(r"^(\d+) boot: boot done")
HELD = re.compile(r"^(\d+) hub loop held ([\d.]+) s on (.*?) \(")
BUSY = re.compile(r"^(\d+) hub loop busy (\d+) s on (.*?) \(")
FREE = re.compile(r"^(\d+) hub loop free after (\d+) s on (.*?) \(")
UNITS = {"m": 60_000, "h": 3_600_000, "d": 86_400_000}
BLOCK_MS = 5_000

# ---- pure part: lines in, runs and holds out --------------------------------


def runs(lines):
    """[{start, booted, end, holds: [(end_ms, held_ms, on)]}] in order; the
    lines before the first `hub start` are left out (no up time known)."""
    out, run, busy = [], None, None

    def close():
        if run is not None and busy is not None:
            run["holds"].append(busy)

    for line in lines:
        m = re.match(r"^(\d+) ", line)
        if not m:
            continue
        ms = int(m.group(1))
        if START.match(line):
            close()
            busy = None
            run = {"start": ms, "booted": None, "end": ms, "holds": []}
            out.append(run)
            continue
        if run is None:
            continue
        run["end"] = ms
        if BOOTED.match(line) and run["booted"] is None:
            run["booted"] = ms
        elif HELD.match(line):
            h = HELD.match(line)
            run["holds"].append((ms, round(float(h.group(2)) * 1000), h.group(3)))
        elif BUSY.match(line):
            b = BUSY.match(line)
            busy = (ms, int(b.group(2)) * 1000, b.group(3))
        elif FREE.match(line):
            f = FREE.match(line)
            run["holds"].append((ms, int(f.group(2)) * 1000, f.group(3)))
            busy = None
    close()
    return out


def summary(rs, since_ms=None):
    """The window from since_ms (None: all): up time, time held and its
    share, and the blocks over 5 s outside a start (a hold that began
    before its run's boot done, or in a run never booted, is a start's)."""
    up = held = 0
    blocks, starts = [], 0
    for r in rs:
        a = r["start"] if since_ms is None else max(r["start"], since_ms)
        if r["end"] <= a:
            continue
        up += r["end"] - a
        for end, ms, on in r["holds"]:
            if end < a:
                continue
            held += min(ms, end - a)
            if ms <= BLOCK_MS:
                continue
            if r["booted"] is None or end - ms < r["booted"]:
                starts += 1
            else:
                blocks.append({"at": end - ms, "ms": ms, "on": on})
    return {
        "up_h": round(up / 3_600_000, 2),
        "held_h": round(held / 3_600_000, 2),
        "held_pct": round(100.0 * held / up, 2) if up else 0.0,
        "blocks_over_5s": len(blocks),
        "worst_ms": max([b["ms"] for b in blocks], default=0),
        "in_starts_over_5s": starts,
        "worst": sorted(blocks, key=lambda b: -b["ms"])[:10],
    }


def since(spec, last_ms):
    """`7d`, `24h`, `30m` before last_ms; None for all."""
    if not spec:
        return None
    m = re.fullmatch(r"(\d+)([mhd])", spec)
    if not m:
        raise SystemExit("--since wants <n>m, <n>h or <n>d: %r" % spec)
    return last_ms - int(m.group(1)) * UNITS[m.group(2)]


def main(argv):
    p = argparse.ArgumentParser(description=__doc__.split(chr(10))[0])
    p.add_argument("log")
    p.add_argument("--since", default="")
    p.add_argument("--json", action="store_true")
    a = p.parse_args(argv)
    with open(a.log, encoding="utf-8", errors="replace") as f:
        rs = runs(f)
    last = max((r["end"] for r in rs), default=0)
    s = summary(rs, since(a.since, last))
    if a.json:
        print(json.dumps(s))
        return 0
    print("up %.1f h, loop held %.2f h: %.2f %% (goal < 1 %%)" % (s["up_h"], s["held_h"], s["held_pct"]))
    print("blocks over 5 s outside a start: %d, worst %.1f s (goal 0); in starts: %d"
          % (s["blocks_over_5s"], s["worst_ms"] / 1000, s["in_starts_over_5s"]))
    for b in s["worst"]:
        print("  %.1f s at %d on %s" % (b["ms"] / 1000, b["at"], b["on"]))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
