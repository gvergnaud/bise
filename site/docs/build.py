#!/usr/bin/env python3
"""bise.dev/docs: turn site/docs/*.md into site/docs/*.html.

    python3 site/docs/build.py            build the pages, search.json, the sitemap rows
    python3 site/docs/build.py --serve    build, then serve site/ with clean URLs (port 4848)
    python3 site/docs/build.py --check    build into memory and fail if a page on disk is stale

Stdlib only. The .md files are the sources and stay served raw, so an agent
can read bise.dev/docs/<page>.md. The site has no build step on deploy: the
generated .html files are committed next to their sources.

Markdown, the subset the pages use:
  front matter     ---  title: ...  description: ...  ---
  headings         ## and ### get an anchor and a row in "on this page"
  code             ```toml title="~/.bise/config.toml"   (title optional)
  lists            - item / 1. item, continuation lines indented
  tables           | a | b |
  callouts         > note: ...   > careful: ...   (careful: only when something can be lost)
  inline           `code`, **bold**, *em*, [text](url)
A page missing from this folder is left out of the sidebar (a draft that
waits for its release lives in docs/site-drafts/).
"""
import html, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
SITE = os.path.dirname(HERE)
REPO_URL = "https://github.com/gvergnaud/bise"
BASE = "https://bise.dev"

# the sidebar: (group, [(slug, label)]); a slug with no .md here is skipped
NAV = [
    ("get started", [("index", "what bise is"), ("install", "install and first run")]),
    ("models", [
        ("providers", "providers and keys"),
        ("subscriptions", "subscriptions"),
        ("models", "models and roles"),
        ("gateways", "gateways and local models"),
    ]),
    ("using bise", [
        ("agents", "main and the agents"),
        ("approvals", "approvals"),
        ("flow", "landing work"),
        ("voice", "voice"),
        ("computer-use", "computer use"),
    ]),
    ("extend", [("plugins", "plugins and MCP"), ("skills", "instructions and skills")]),
    ("reference", [
        ("config", "config.toml"),
        ("cli", "the bise command"),
        ("keys", "keys and commands"),
        ("troubleshooting", "troubleshooting"),
        ("updates", "updates"),
    ]),
]


# ---------- markdown ----------

def slugify(s):
    s = re.sub(r"<[^>]+>", "", s)
    s = html.unescape(s).lower()
    s = re.sub(r"[^a-z0-9 _-]", "", s)
    return re.sub(r"[\s_]+", "-", s.strip()).strip("-") or "section"


KEY_RE = re.compile(r"^(/[a-z][a-z-]*( [a-z|<>\[\]-]+)*|(ctrl|shift|cmd|alt|⌥|⇧)\+.+|esc|tab|⏎|enter|space|hold space|ctrl\+r twice)$")


def inline(s, page_links):
    """Inline Markdown to HTML. Code spans first, so nothing inside them is touched."""
    codes = []

    def keep_code(m):
        t = m.group(1)
        cls = "key" if KEY_RE.match(t) else ""
        codes.append(f'<code class="{cls}">{html.escape(t)}</code>' if cls else f"<code>{html.escape(t)}</code>")
        return f"\x00{len(codes) - 1}\x00"

    s = re.sub(r"`([^`]+)`", keep_code, s)
    s = html.escape(s, quote=False)

    def link(m):
        text, url = m.group(1), m.group(2)
        ext = url.startswith("http")
        if not ext and not url.startswith(("/", "#", "mailto:")):
            page, _, frag = url.partition("#")
            page_links.append(page)
            url = "/docs" + ("" if page == "index" else "/" + page) + ("#" + frag if frag else "")
        attr = ' rel="noopener"' if ext else ""
        return f'<a href="{html.escape(url, quote=True)}"{attr}>{text}</a>'

    s = re.sub(r"\[([^\]]+)\]\(([^)\s]+)\)", link, s)
    s = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", s)
    s = re.sub(r"(?<![\w*])\*([^*\s][^*]*?)\*(?![\w*])", r"<em>\1</em>", s)
    return re.sub(r"\x00(\d+)\x00", lambda m: codes[int(m.group(1))], s)


COMMENT = {"toml": "#", "sh": "#", "bash": "#", "text": None, "json": None, "": None, "yaml": "#", "py": "#"}


def code_block(lang, title, lines):
    mark = COMMENT.get(lang, "#")
    out = []
    for ln in lines:
        e = html.escape(ln)
        if mark:
            # a comment: a whole line, or after two spaces (not inside a string, good enough here)
            m = re.match(r"^(\s*)(#.*)$", ln) or re.match(r'^([^"]*?(?:"[^"]*"[^"]*?)*?\s)(#\s.*)$', ln)
            if m:
                e = html.escape(m.group(1)) + f'<span class="c">{html.escape(m.group(2))}</span>'
        out.append(e)
    body = "\n".join(out)
    label = f'<div class="file">{html.escape(title)}</div>' if title else ""
    return (f'<div class="code">{label}<div class="box"><pre><code class="lang-{html.escape(lang or "text")}">{body}</code></pre>'
            f'<button class="copy" type="button" aria-label="copy the code">copy</button></div></div>')


def table(rows, links):
    def cells(r):
        r = r.strip().strip("|")
        return [c.strip() for c in re.split(r"(?<!\\)\|", r)]

    head = cells(rows[0])
    body = [cells(r) for r in rows[2:]]
    h = "".join(f"<th>{inline(c, links)}</th>" for c in head)
    b = "".join("<tr>" + "".join(f"<td>{inline(c.replace(chr(92) + '|', '|'), links)}</td>" for c in r) + "</tr>" for r in body)
    return f'<div class="table"><table><thead><tr>{h}</tr></thead><tbody>{b}</tbody></table></div>'


def render(md):
    """Markdown body to (html, toc, plain sections for search, linked pages)."""
    lines = md.split("\n")
    out, toc, sections, links = [], [], [], []
    cur = {"heading": "", "anchor": "", "text": []}
    seen = set()
    i = 0

    def text_of(s):
        cur["text"].append(re.sub(r"[`*\[\]]|\]\([^)]*\)", "", s))

    while i < len(lines):
        ln = lines[i]
        if not ln.strip():
            i += 1
            continue
        m = re.match(r"^```(\w*)(?:\s+title=\"([^\"]+)\")?\s*$", ln)
        if m:
            j = i + 1
            while j < len(lines) and not lines[j].startswith("```"):
                j += 1
            out.append(code_block(m.group(1), m.group(2), lines[i + 1:j]))
            text_of(" ".join(lines[i + 1:j]))
            i = j + 1
            continue
        m = re.match(r"^(#{2,4})\s+(.*)$", ln)
        if m:
            level, title = len(m.group(1)), m.group(2).strip()
            a = slugify(title)
            n, base = 2, a
            while a in seen:
                a, n = f"{base}-{n}", n + 1
            seen.add(a)
            hh = inline(title, links)
            out.append(f'<h{level} id="{a}"><a class="anchor" href="#{a}" aria-hidden="true">#</a>{hh}</h{level}>')
            if level <= 3:
                toc.append((level, a, re.sub(r"<[^>]+>", "", hh)))
            sections.append(cur)
            cur = {"heading": html.unescape(re.sub(r"<[^>]+>", "", hh)), "anchor": a, "text": []}
            i += 1
            continue
        if ln.startswith("|") and i + 1 < len(lines) and re.match(r"^\|[\s:|-]+\|\s*$", lines[i + 1]):
            j = i
            while j < len(lines) and lines[j].startswith("|"):
                j += 1
            out.append(table(lines[i:j], links))
            text_of(" ".join(lines[i:j]).replace("|", " "))
            i = j
            continue
        if ln.startswith(">"):
            j, buf = i, []
            while j < len(lines) and lines[j].startswith(">"):
                buf.append(lines[j][1:].strip())
                j += 1
            t = " ".join(buf)
            m = re.match(r"^(note|careful):\s*(.*)$", t, re.S)
            if m:
                kind, body = m.group(1), m.group(2)
                label = "▲ careful" if kind == "careful" else "note"
                out.append(f'<aside class="callout {kind}"><span class="label">{label}</span><p>{inline(body, links)}</p></aside>')
            else:
                out.append(f"<blockquote><p>{inline(t, links)}</p></blockquote>")
            text_of(t)
            i = j
            continue
        m = re.match(r"^(\s*)([-*]|\d+\.)\s+(.*)$", ln)
        if m:
            ordered = m.group(2)[0].isdigit()
            items, j = [], i
            while j < len(lines):
                mm = re.match(r"^([-*]|\d+\.)\s+(.*)$", lines[j])
                if mm:
                    items.append(mm.group(2))
                    j += 1
                elif lines[j].startswith(("  ", "\t")) and lines[j].strip() and items:
                    items[-1] += " " + lines[j].strip()
                    j += 1
                elif not lines[j].strip() and j + 1 < len(lines) and re.match(r"^([-*]|\d+\.)\s+", lines[j + 1]):
                    j += 1
                else:
                    break
            tag = "ol" if ordered else "ul"
            out.append(f"<{tag}>" + "".join(f"<li>{inline(it, links)}</li>" for it in items) + f"</{tag}>")
            for it in items:
                text_of(it)
            i = j
            continue
        j, buf = i, []
        while j < len(lines) and lines[j].strip() and not re.match(r"^(```|#{2,4}\s|\||>|([-*]|\d+\.)\s)", lines[j]):
            buf.append(lines[j].strip())
            j += 1
        if not buf:  # a line no rule took: keep it as text
            buf, j = [ln.strip()], i + 1
        p = " ".join(buf)
        out.append(f"<p>{inline(p, links)}</p>")
        text_of(p)
        i = j
    sections.append(cur)
    return "\n".join(out), toc, sections, links


def when(md, pages):
    """`<!-- if PAGE -->...<!-- end -->`: kept only when PAGE is a page here,
    so a page can mention a draft (docs/site-drafts/) that goes live later."""
    def keep(m):
        return m.group(2) if m.group(1) in pages else ""
    return re.sub(r"<!-- if ([a-z0-9-]+) -->(.*?)<!-- end -->", keep, md, flags=re.S)


def front(md):
    meta = {}
    if md.startswith("---\n"):
        end = md.index("\n---\n", 4)
        for ln in md[4:end].split("\n"):
            k, _, v = ln.partition(":")
            meta[k.strip()] = v.strip()
        md = md[end + 5:]
    return meta, md


# ---------- page ----------

def url_of(slug):
    return "/docs" if slug == "index" else f"/docs/{slug}"


def sidebar(pages, current):
    out = []
    for group, items in NAV:
        rows = [(s, l) for s, l in items if s in pages]
        if not rows:
            continue
        out.append(f'<div class="grp">{html.escape(group)}</div><ul>')
        for s, l in rows:
            cls = ' class="here" aria-current="page"' if s == current else ""
            out.append(f'<li><a href="{url_of(s)}"{cls}>{html.escape(l)}</a></li>')
        out.append("</ul>")
    return "\n".join(out)


def order(pages):
    return [s for _, items in NAV for s, _ in items if s in pages]


TEMPLATE = open(os.path.join(HERE, "page.tmpl"), encoding="utf-8").read()


def page_html(slug, meta, body, toc, pages):
    seq = order(pages)
    k = seq.index(slug)
    prev = seq[k - 1] if k > 0 else None
    nxt = seq[k + 1] if k + 1 < len(seq) else None
    label = {s: l for _, items in NAV for s, l in items}
    pn = ""
    if prev:
        pn += f'<a class="prev" href="{url_of(prev)}"><span>previous</span>{html.escape(label[prev])}</a>'
    if nxt:
        pn += f'<a class="next" href="{url_of(nxt)}"><span>next</span>{html.escape(label[nxt])}</a>'
    toc_html = "".join(f'<li class="l{lv}"><a href="#{a}">{t}</a></li>' for lv, a, t in toc)
    title = meta.get("title", slug)
    md_url = f"{BASE}/docs/{slug}.md"
    rep = {
        "{{title}}": html.escape(title),
        "{{head_title}}": html.escape(("bise docs" if slug == "index" else f"{title} · bise docs")),
        "{{description}}": html.escape(meta.get("description", "")),
        "{{canonical}}": BASE + url_of(slug),
        "{{sidebar}}": sidebar(pages, slug),
        "{{body}}": body,
        "{{toc}}": f'<div class="toch">on this page</div><ul>{toc_html}</ul>' if toc else "",
        "{{prevnext}}": pn,
        "{{md}}": f"/docs/{slug}.md",
        "{{md_url}}": md_url.replace("https://", ""),
        "{{edit}}": f"{REPO_URL}/blob/main/site/docs/{slug}.md",
    }
    out = TEMPLATE
    for a, b in rep.items():
        out = out.replace(a, b)
    return out


def build():
    pages = {}
    for f in sorted(os.listdir(HERE)):
        if f.endswith(".md") and f != "README.md":
            pages[f[:-3]] = open(os.path.join(HERE, f), encoding="utf-8").read()
    unlisted = [s for s in pages if s not in {s for _, it in NAV for s, _ in it}]
    if unlisted:
        sys.exit(f"pages not in NAV: {', '.join(unlisted)} (add them to build.py's NAV)")
    files, index, problems = {}, [], []
    for slug, src in pages.items():
        meta, md = front(src)
        md = when(md, pages)
        if "title" not in meta or "description" not in meta:
            problems.append(f"{slug}.md: front matter needs title and description")
        body, toc, sections, links = render(md)
        for l in links:
            if l and l not in pages:
                problems.append(f"{slug}.md links to '{l}', which is not a page here")
        files[f"{slug}.html"] = page_html(slug, meta, body, toc, pages)
        for s in sections:
            t = " ".join(" ".join(s["text"]).split())
            if not t and not s["heading"]:
                continue
            index.append({"p": url_of(slug), "t": meta.get("title", slug), "h": s["heading"], "a": s["anchor"], "x": t[:1200]})
    files["search.json"] = json.dumps(index, ensure_ascii=False, separators=(",", ":"))
    return files, problems, pages


def sitemap(pages):
    path = os.path.join(SITE, "sitemap.xml")
    sm = open(path, encoding="utf-8").read()
    sm = re.sub(r"  <url><loc>https://bise\.dev/docs[^<]*</loc>.*\n", "", sm)
    rows = "".join(f"  <url><loc>{BASE}{url_of(s)}</loc><changefreq>weekly</changefreq><priority>{'0.8' if s == 'index' else '0.6'}</priority></url>\n" for s in order(pages))
    return sm.replace("</urlset>", rows + "</urlset>")


def main():
    files, problems, pages = build()
    if problems:
        sys.exit("\n".join(problems))
    sm = sitemap(pages)
    if "--check" in sys.argv:
        stale = [f for f, c in files.items() if not os.path.exists(os.path.join(HERE, f)) or open(os.path.join(HERE, f), encoding="utf-8").read() != c]
        if open(os.path.join(SITE, "sitemap.xml"), encoding="utf-8").read() != sm:
            stale.append("../sitemap.xml")
        if stale:
            sys.exit("stale: " + ", ".join(stale) + " (run python3 site/docs/build.py)")
        print(f"ok: {len(pages)} pages up to date")
        return
    for f, c in files.items():
        with open(os.path.join(HERE, f), "w", encoding="utf-8") as fh:
            fh.write(c)
    for f in os.listdir(HERE):  # a page that was removed: its html goes too
        if f.endswith(".html") and f[:-5] not in pages:
            os.remove(os.path.join(HERE, f))
    with open(os.path.join(SITE, "sitemap.xml"), "w", encoding="utf-8") as fh:
        fh.write(sm)
    print(f"built {len(pages)} pages")
    if "--serve" in sys.argv:
        serve()


def serve():
    import http.server

    class H(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *a, **k):
            super().__init__(*a, directory=SITE, **k)

        def end_headers(self):
            self.send_header("Cache-Control", "no-store")
            super().end_headers()

        def guess_type(self, path):
            return "text/plain; charset=utf-8" if path.endswith(".md") else super().guess_type(path)

        def do_GET(self):  # Vercel's cleanUrls: /docs/x -> docs/x.html
            p = self.path.split("?")[0].split("#")[0]
            disk = os.path.join(SITE, p.lstrip("/"))
            if not os.path.exists(disk) and os.path.exists(disk + ".html"):
                self.path = p + ".html"
            elif os.path.isdir(disk) and not p.endswith("/"):
                self.path = p + "/index.html"
            super().do_GET()

    port = int(os.environ.get("PORT", "4848"))
    print(f"http://localhost:{port}/docs")
    http.server.ThreadingHTTPServer(("127.0.0.1", port), H).serve_forever()


if __name__ == "__main__":
    main()
