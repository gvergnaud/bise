#!/usr/bin/env python3
"""The release's built-in plugins are exactly the shipped list (architect's merge review, B1).

scripts/versions.sh copies the whole repo `plugins/` into a version's app root, and bend_plugins
(rust/plugins/src/resolve.rs builtin_root) loads that folder as built-in, on for every user. So
whatever sits in `plugins/` ships. Only `computer`, the office plugins (`sheets`, `slides`,
`docs`: docs/office.md, plan §3), the viewer apps (`media`'s Preview, 6a1668b59; `sites`,
9d2806845), and the gallery's optional plugins (maps, draw, mermaid, dataviz: page
plugin-gallery) may be there (architect m_63842); each gallery plugin is off until he turns it on
(`"dev.bise": {"default": "off"}`), and their shared view kit `plugins/.view-kit/` ships with them
(its servers inline it at resources/read) as the only dot folder. The repo's own QA plugin lives in
`dev/plugins/ambient-qa`, which versions.sh never copies (not .agents/plugins: the main checkout's
.git/info/exclude hides that folder, so a land there silently dropped every file, 4e34639a).
Every type the host stopped drawing (DROPPED: the built-in viewers the plugin-apps steps deleted,
docs/plugin-apps.md A3's table) must open in a shipped, default-on plugin's view: a `files` entry
of its view.json's `dev.bise/ui`. Else a released bise can't show it.
Offline, no build: `python3 tests/release_plugins_test.py`."""
import json
import os
import re
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
GALLERY = ["dataviz", "draw", "maps", "mermaid"]
SHIPPED = sorted(["computer", "docs", "sheets", "slides", "media", "sites"] + GALLERY)
KIT = ["kit.css", "kit.js", "inline.mjs"]
# The types bise's built-in viewers drew and no longer draw: steps 1-5 of docs/plugin-apps.md
# (Preview 6a1668b59, Writer, Slides, Sheets, Sites 9d2806845) deleted them and the matching
# BUILTIN rows of rust/plugins/src/opens.rs. No machine list of them is left anywhere else
# (architect m_63842). A `*.page` is still BUILTIN (step 6): drawn by the host, not here.
DROPPED = ["*.png", "*.jpg", "*.jpeg", "*.gif", "*.webp", "*.pdf", "*.mp4", "*.mov", "*.webm", "*.m4v",
           "*.md", "*.markdown", "*.docx", "*.pptx", "*.xlsx", "*.csv", "*.html", "*.htm", "*/index.html"]


def default_on(root, name):
    man = os.path.join(root, name, "plugin.json")
    if not os.path.isfile(man):
        return False
    ext = (json.load(open(man)).get("extensions") or {}).get("dev.bise") or {}
    return ext.get("default") != "off"


def opened_types(root, plugins):
    """{type key: [plugin/view]} over the default-on plugins' static views."""
    out = {}
    for p in plugins:
        if not default_on(root, p):
            continue
        views = os.path.join(root, p, "ai.mistral.vibe", "views")
        for v in sorted(os.listdir(views)) if os.path.isdir(views) else []:
            vj = os.path.join(views, v, "view.json")
            if not os.path.isfile(vj):
                continue
            ui = (json.load(open(vj)).get("_meta") or {}).get("dev.bise/ui") or {}
            for entry in ui.get("files") or []:
                for key in entry.get("match") or []:
                    out.setdefault(key, []).append("%s/%s" % (p, v))
    return out


def main():
    fails = []
    root = os.path.join(REPO, "plugins")
    plugins = sorted(n for n in os.listdir(root) if not n.startswith("."))
    # a gallery plugin still being built is not there yet: never one outside the list
    extra = [n for n in plugins if n not in SHIPPED]
    if extra or not set(SHIPPED) - set(GALLERY) <= set(plugins):
        fails.append("plugins/ holds %s, the release ships at most %s (an internal plugin goes in dev/plugins/)"
                     % (plugins, SHIPPED))
    dots = sorted(n for n in os.listdir(root) if n.startswith(".") and n != ".DS_Store")
    if dots != [".view-kit"]:
        fails.append("plugins/ dot folders are %s, only .view-kit ships" % dots)
    for f in KIT:
        if not os.path.isfile(os.path.join(root, ".view-kit", f)):
            fails.append("plugins/.view-kit/%s is missing: the gallery's views would draw unstyled" % f)
    for n in GALLERY:
        man = os.path.join(root, n, "plugin.json")
        if os.path.isfile(man):
            ext = (json.load(open(man)).get("extensions") or {}).get("dev.bise") or {}
            if ext.get("default") != "off":
                fails.append("plugins/%s is a gallery plugin: its plugin.json must say \"dev.bise\": {\"default\": \"off\"}" % n)
    opens = opened_types(root, [n for n in plugins if n in SHIPPED])
    for key in DROPPED:
        if key not in opens:
            fails.append("%s: bise no longer draws it and no shipped default-on plugin's view opens it "
                         "(a files entry in plugins/<p>/ai.mistral.vibe/views/<v>/view.json)" % key)
    versions = open(os.path.join(REPO, "scripts", "versions.sh")).read()
    if not re.search(r'cp -R "\$src/plugins" "\$tmp/plugins"', versions):
        fails.append("scripts/versions.sh no longer copies plugins/ as a whole: check this law still guards what ships")
    if re.search(r"\$src/dev\b", versions):
        fails.append("scripts/versions.sh mentions dev/: the repo's dev plugins must never ship")
    qa = os.path.join(REPO, "dev", "plugins", "ambient-qa", "plugin.json")
    if not os.path.isfile(qa):
        fails.append("no %s: the QA plugin moved without this law" % os.path.relpath(qa, REPO))
    for f in fails:
        print("FAIL", f)
    print("%d failed" % len(fails) if fails else "release plugins: %s" % plugins)
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
