//! The `ui` block (pages-ui, main m_7432): what the kit lets a page draw
//! beyond text, a feature proposal's real terminal cells, states side by
//! side, an option picker. Inside a `<section data-kit="ui">` the lint also
//! takes `div span figure button svg…`, `class`, kit behaviours named by
//! `data-ui`, and one `<style>` whose rules apply to that block only.
//!
//! The CSP stays `'self'`: the page server never sends a `<style>` in the
//! page. It serves the scoped rules of a version as `/p/<id>/v/<n>/page.css`
//! ([`page_css`]) and strips the `<style>` elements from the fragment
//! ([`strip_styles`]). The rules keep a page from covering or faking the
//! frame, the notes or the send buttons (ambient-lead m_7450): every
//! selector is prefixed with the block, no `:root`/`html`/`body`/id
//! selectors or the kit's own classes, no `url()`, `@import`,
//! `@font-face`, `position: fixed|sticky`, no z-index above 9. And the look
//! stays bise's (ambient m_7451): colors, fonts and font sizes only through
//! the kit's tokens (`var(--…)`).
//!
//! Pure: no I/O.

/// The elements a ui block takes on top of the text ones.
pub const ELEMENTS: &[&str] = &[
    "div", "span", "figure", "figcaption", "button", "details", "summary", "svg", "g", "path",
    "rect", "circle", "ellipse", "line", "polyline", "polygon", "text", "tspan", "title", "style",
];

/// The kit behaviours a `data-ui` names (kit.js): tabs over panes, a
/// terminal frame's light/dark switch, an option picker whose pick is a note.
pub const BEHAVIOURS: &[&str] = &["tabs", "term", "pick"];

/// Attributes of SVG drawing, values checked for colors.
const SVG_ATTRS: &[&str] = &[
    "viewbox", "d", "x", "y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "rx", "ry", "width", "height",
    "points", "transform", "fill", "stroke", "stroke-width", "stroke-linecap", "stroke-linejoin",
    "stroke-dasharray", "opacity", "fill-opacity", "stroke-opacity", "text-anchor", "dominant-baseline",
    "preserveaspectratio", "xmlns", "focusable", "font-size", "font-weight",
];

/// The checks of an attribute inside a ui block: `None` when the ui block
/// has nothing to say about it (the common checks apply), `Some(Ok)` when it
/// takes it, `Some(Err(fix))` when it refuses it.
pub fn attr(el: &str, name: &str, value: &str) -> Option<Result<(), String>> {
    let n = name;
    if n == "class" {
        let ok = value.split_whitespace().all(|c| {
            !c.is_empty()
                && c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
                && !reserved_class(c)
        });
        return Some(if ok {
            Ok(())
        } else {
            Err(format!("class=\"{value}\": class names are letters, digits, - and _, and never the kit's own (bise-, bn-, kit-)"))
        });
    }
    if n == "data-ui" {
        return Some(if BEHAVIOURS.contains(&value) {
            Ok(())
        } else {
            Err(format!("data-ui=\"{value}\" is not a kit behaviour: use {}", BEHAVIOURS.join(", ")))
        });
    }
    if let Some(rest) = n.strip_prefix("data-") {
        // the kit's own marks: the block's id and kind, the decorations kit.js adds
        return Some(match rest {
            "kit" | "id" | "kit-ui" | "verdict" | "picked" | "went" => Err(format!(
                "{n} is the kit's: inside a ui block name an option data-pick=\"a\", a pane data-tab=\"dark\""
            )),
            _ if rest.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') => Ok(()),
            _ => Err(format!("{n}: data- names are lowercase letters, digits and -")),
        });
    }
    if n.starts_with("aria-") || matches!(n, "role" | "hidden" | "tabindex") {
        return Some(Ok(()));
    }
    if el == "button" && n == "type" {
        return Some(if value == "button" { Ok(()) } else { Err("a button is type=\"button\": pages carry no forms".into()) });
    }
    if el == "details" && n == "open" {
        return Some(Ok(()));
    }
    if SVG_ATTRS.contains(&n) {
        if matches!(n, "fill" | "stroke") && !matches!(value.trim(), "none" | "currentColor" | "currentcolor" | "transparent") {
            return Some(Err(format!("{n}=\"{value}\": use none or currentColor, and color it with a class and var(--…)")));
        }
        return Some(Ok(()));
    }
    None
}

fn reserved_class(c: &str) -> bool {
    let c = c.to_ascii_lowercase();
    c.starts_with("bise") || c.starts_with("bn-") || c.starts_with("kit-")
}

/// The problems of a ui block's `<style>`, one line each (empty: fine).
pub fn check_css(css: &str) -> Vec<String> {
    let mut errs = Vec::new();
    walk(css, &mut |ev| match ev {
        Css::At(name) => {
            if !matches!(name.as_str(), "media" | "supports" | "container" | "keyframes" | "-webkit-keyframes") {
                errs.push(format!("@{name} is not allowed in a ui block: @media, @supports, @container and @keyframes are"));
            }
        }
        Css::Selector(s) => {
            if let Some(why) = bad_selector(&s) {
                errs.push(why);
            }
        }
        Css::Decl(p, v) => {
            if let Some(why) = bad_decl(&p, &v) {
                errs.push(why);
            }
        }
        Css::Broken(why) => errs.push(why),
    });
    let mut seen = Vec::new();
    errs.retain(|e| {
        let new = !seen.contains(e);
        seen.push(e.clone());
        new
    });
    errs
}

/// A ui block's rules, scoped to it: every selector prefixed with
/// `[data-id="<id>"]` (`:scope` is the block itself). Comments go; the
/// rules are assumed checked ([`check_css`]).
pub fn scope_css(id: &str, css: &str) -> String {
    let pre = format!("[data-id=\"{id}\"]");
    let mut out = String::new();
    let src = strip_comments(css);
    scope_into(&pre, &src, &mut out);
    out
}

/// Every ui block's `<style>` of a fragment, scoped, in document order:
/// the version's page.css.
pub fn page_css(html: &str) -> String {
    let mut out = String::new();
    for (id, css) in styles(html) {
        if check_css(&css).is_empty() {
            out.push_str(&format!("/* {id} */\n"));
            out.push_str(&scope_css(&id, &css));
            out.push('\n');
        }
    }
    out
}

/// The fragment without its `<style>` elements (they are served as
/// page.css; inline, the CSP would refuse them anyway).
pub fn strip_styles(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let low = html.to_ascii_lowercase();
    let mut i = 0;
    while let Some(s) = find_tag(&low, i, "style") {
        out.push_str(&html[i..s]);
        i = match low[s..].find("</style") {
            Some(e) => low[s + e..].find('>').map_or(html.len(), |g| s + e + g + 1),
            None => html.len(),
        };
    }
    out.push_str(&html[i..]);
    out
}

/// (block id, css) of each `<style>` inside a `<section data-kit="ui">`.
pub fn styles(html: &str) -> Vec<(String, String)> {
    let low = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(s) = find_tag(&low, i, "section") {
        let open_end = low[s..].find('>').map_or(low.len(), |e| s + e + 1);
        let end = low[open_end..].find("</section").map_or(low.len(), |e| open_end + e);
        let head = &html[s..open_end];
        if attr_of(head, "data-kit").as_deref() == Some("ui") {
            if let Some(id) = attr_of(head, "data-id") {
                let mut j = open_end;
                while let Some(t) = find_tag(&low[..end], j, "style") {
                    let body = low[t..].find('>').map_or(end, |g| t + g + 1);
                    let close = low[body..end].find("</style").map_or(end, |e| body + e);
                    out.push((id.clone(), html[body..close].to_string()));
                    j = close.max(body);
                    if j >= end {
                        break;
                    }
                }
            }
        }
        i = end.max(s + 1);
    }
    out
}

/// The next `<name` tag (followed by a space, `/` or `>`) from `from`.
fn find_tag(low: &str, from: usize, name: &str) -> Option<usize> {
    let pat = format!("<{name}");
    let mut i = from;
    while let Some(p) = low.get(i..)?.find(&pat) {
        let at = i + p;
        let next = low.as_bytes().get(at + pat.len()).copied().unwrap_or(b'>');
        if next.is_ascii_whitespace() || next == b'>' || next == b'/' {
            return Some(at);
        }
        i = at + pat.len();
    }
    None
}

/// An attribute's value in an open tag (`<section data-kit="ui" …>`).
fn attr_of(tag: &str, name: &str) -> Option<String> {
    let low = tag.to_ascii_lowercase();
    let mut i = 0;
    while let Some(p) = low[i..].find(name) {
        let at = i + p;
        let before = low.as_bytes().get(at.wrapping_sub(1)).copied().unwrap_or(b' ');
        let rest = low[at + name.len()..].trim_start();
        if before.is_ascii_whitespace() && rest.starts_with('=') {
            let v = rest[1..].trim_start();
            let off = tag.len() - v.len();
            let q = v.as_bytes().first().copied()?;
            return if q == b'"' || q == b'\'' {
                tag[off + 1..].find(q as char).map(|e| tag[off + 1..off + 1 + e].to_string())
            } else {
                Some(tag[off..].split(|c: char| c.is_whitespace() || c == '>').next()?.to_string())
            };
        }
        i = at + name.len();
    }
    None
}

// ---- a small CSS reader: enough to check and scope what the guide teaches ----

enum Css {
    At(String),
    Selector(String),
    Decl(String, String),
    Broken(String),
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(s) = rest.find("/*") {
        out.push_str(&rest[..s]);
        rest = rest[s + 2..].find("*/").map_or("", |e| &rest[s + 2 + e + 2..]);
    }
    out.push_str(rest);
    out
}

/// The index of the first of `stops` at depth 0 outside strings, from `i`.
fn scan(s: &str, i: usize, stops: &[u8]) -> Option<usize> {
    let b = s.as_bytes();
    let (mut j, mut depth, mut quote) = (i, 0i32, 0u8);
    while j < b.len() {
        let c = b[j];
        if quote != 0 {
            if c == b'\\' {
                j += 1;
            } else if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'(' || c == b'[' {
            depth += 1;
        } else if c == b')' || c == b']' {
            depth -= 1;
        } else if depth == 0 && stops.contains(&c) {
            return Some(j);
        }
        j += 1;
    }
    None
}

/// The `}` that closes the block whose `{` is at `open`.
fn close_of(s: &str, open: usize) -> Option<usize> {
    let mut depth = 0;
    let mut j = open;
    loop {
        let k = scan(s, j, b"{}")?;
        if s.as_bytes()[k] == b'{' {
            depth += 1;
        } else {
            depth -= 1;
            if depth == 0 {
                return Some(k);
            }
        }
        j = k + 1;
    }
}

fn walk(css: &str, f: &mut dyn FnMut(Css)) {
    let src = strip_comments(css);
    walk_rules(&src, f);
}

fn walk_rules(s: &str, f: &mut dyn FnMut(Css)) {
    let mut i = 0;
    while i < s.len() {
        let Some(open) = scan(s, i, b"{};") else {
            if !s[i..].trim().is_empty() {
                f(Css::Broken(format!("\"{}\" is not a rule: write selector {{ property: value }}", s[i..].trim())));
            }
            return;
        };
        let pre = s[i..open].trim();
        match s.as_bytes()[open] {
            b';' => {
                // a statement at-rule (@import x;) or a stray declaration
                if let Some(name) = pre.strip_prefix('@') {
                    f(Css::At(name.split_whitespace().next().unwrap_or("").to_ascii_lowercase()));
                } else if !pre.is_empty() {
                    f(Css::Broken(format!("\"{pre}\" is outside a rule: put it in selector {{ … }}")));
                }
                i = open + 1;
            }
            b'}' => {
                f(Css::Broken("a } without its {".into()));
                i = open + 1;
            }
            _ => {
                let Some(close) = close_of(s, open) else {
                    f(Css::Broken(format!("\"{pre}\" is not closed: end it with }}")));
                    return;
                };
                let body = &s[open + 1..close];
                if let Some(at) = pre.strip_prefix('@') {
                    let name = at.split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("").to_ascii_lowercase();
                    f(Css::At(name.clone()));
                    if name.ends_with("keyframes") {
                        walk_keyframes(body, f);
                    } else {
                        walk_rules(body, f);
                    }
                } else {
                    for sel in split_top(pre, b',') {
                        f(Css::Selector(sel.trim().to_string()));
                    }
                    walk_decls(body, f);
                }
                i = close + 1;
            }
        }
    }
}

fn walk_keyframes(s: &str, f: &mut dyn FnMut(Css)) {
    let mut i = 0;
    while let Some(open) = scan(s, i, b"{") {
        let Some(close) = close_of(s, open) else { return };
        walk_decls(&s[open + 1..close], f);
        i = close + 1;
    }
}

fn walk_decls(s: &str, f: &mut dyn FnMut(Css)) {
    for d in split_top(s, b';') {
        let d = d.trim();
        if d.is_empty() {
            continue;
        }
        if d.contains('{') {
            f(Css::Broken("a rule inside a rule: ui blocks take flat rules (no nesting)".into()));
            continue;
        }
        match d.split_once(':') {
            Some((p, v)) => f(Css::Decl(p.trim().to_ascii_lowercase(), v.trim().to_string())),
            None => f(Css::Broken(format!("\"{d}\" is not a declaration: write property: value"))),
        }
    }
}

fn split_top(s: &str, sep: u8) -> Vec<&str> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(k) = scan(s, i, &[sep]) {
        out.push(&s[i..k]);
        i = k + 1;
    }
    out.push(&s[i..]);
    out
}

fn bad_selector(sel: &str) -> Option<String> {
    let low = sel.to_ascii_lowercase();
    if low.is_empty() {
        return Some("an empty selector: name what it styles (.row, :scope)".into());
    }
    let words: Vec<&str> = low
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .filter(|w| !w.is_empty())
        .collect();
    let element = |name: &str| {
        // `html` as an element (not inside a class or attribute name)
        low.match_indices(name).any(|(at, _)| {
            let before = low[..at].chars().last();
            let after = low[at + name.len()..].chars().next();
            !matches!(before, Some(c) if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '#' | ':' | '[' | '='))
                && !matches!(after, Some(c) if c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        })
    };
    if low.contains(":root") || element("html") || element("body") || element("main") || element("section") {
        return Some(format!("selector \"{sel}\" reaches out of the block: style what is inside it (:scope is the block)"));
    }
    if low.contains('#') {
        return Some(format!("selector \"{sel}\": no id selectors, use a class"));
    }
    if low.contains("[data-kit") || low.contains("[data-id") || words.iter().any(|w| reserved_class(w) && low.contains(&format!(".{w}"))) {
        return Some(format!("selector \"{sel}\" names the kit's own parts: style your classes"));
    }
    None
}

/// Named colors an agent might write instead of a token.
const NAMED: &[&str] = &[
    "white", "black", "red", "green", "blue", "yellow", "orange", "purple", "pink", "gray", "grey",
    "brown", "cyan", "magenta", "navy", "teal", "olive", "maroon", "silver", "gold", "lime", "aqua",
    "fuchsia", "indigo", "violet", "salmon", "coral", "crimson", "tomato", "beige", "ivory",
];

fn bad_decl(p: &str, v: &str) -> Option<String> {
    let low = v.to_ascii_lowercase();
    let at = format!("{p}: {v}");
    if low.contains("url(") || low.contains("image-set(") || low.contains("expression(") {
        return Some(format!("{at}: no url() in a ui block, draw it with the kit's components or an <svg>"));
    }
    if p == "behavior" || p == "-moz-binding" {
        return Some(format!("{at}: not allowed"));
    }
    if p == "position" && (low.contains("fixed") || low.contains("sticky")) {
        return Some(format!("{at}: a ui block stays in the page: use position: relative or absolute"));
    }
    if p == "z-index" && low.trim().parse::<i64>().is_ok_and(|z| z > 9) {
        return Some(format!("{at}: z-index 9 at most, the frame and the notes stay on top"));
    }
    if p == "content" && !low.trim_start().starts_with('"') && !low.trim_start().starts_with('\'') && !matches!(low.trim(), "none" | "normal" | "''" | "\"\"") && !low.contains("attr(") && !low.contains("counter") {
        return Some(format!("{at}: content takes a quoted string, attr() or a counter"));
    }
    // colors only through the tokens (ambient m_7451)
    let unquoted = strip_strings(&low);
    if unquoted.contains('#') || ["rgb(", "rgba(", "hsl(", "hsla(", "hwb(", "lab(", "lch(", "oklab(", "oklch(", "color("].iter().any(|f| unquoted.contains(f)) {
        return Some(format!("{at}: colors come from the kit's tokens: var(--ink), var(--dim), var(--accent), var(--term-…)"));
    }
    if unquoted
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .any(|w| NAMED.contains(&w))
        && !p.starts_with("--")
        && p != "animation-name"
        && p != "animation"
    {
        return Some(format!("{at}: colors come from the kit's tokens: var(--ink), var(--dim), var(--accent), var(--term-…)"));
    }
    if p == "font-family" && !low.trim().starts_with("var(--font-") && !matches!(low.trim(), "inherit") {
        return Some(format!("{at}: the type is the kit's: var(--font-read), var(--font-ui), var(--font-mono) or var(--font-display)"));
    }
    if p == "font" && !low.contains("var(--font-") && !matches!(low.trim(), "inherit") {
        return Some(format!("{at}: write font-family: var(--font-…) and font-size: var(--text-…)"));
    }
    if (p == "font-size" || p == "font") && has_unit(&unquoted, &["px", "pt", "vw", "vh"]) {
        return Some(format!("{at}: font sizes come from the scale: var(--text-read), var(--text-small), var(--text-ui), var(--text-kicker), or em"));
    }
    if matches!(p, "width" | "min-width" | "max-width") && px_over(&unquoted, 1240) {
        return Some(format!("{at}: wider than a page: use 100% or a share of it"));
    }
    None
}

fn strip_strings(s: &str) -> String {
    let mut out = String::new();
    let mut quote = None;
    for c in s.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None => out.push(c),
        }
    }
    out
}

fn has_unit(v: &str, units: &[&str]) -> bool {
    let b = v.as_bytes();
    units.iter().any(|u| {
        v.match_indices(u).any(|(at, _)| {
            at > 0
                && b[at - 1].is_ascii_digit()
                && !b.get(at + u.len()).is_some_and(|c| c.is_ascii_alphanumeric())
        })
    })
}

fn px_over(v: &str, max: u64) -> bool {
    v.match_indices("px").any(|(at, _)| {
        let digits: String = v[..at].chars().rev().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        let n: f64 = digits.chars().rev().collect::<String>().parse().unwrap_or(0.0);
        n > max as f64
    })
}

fn scope_into(pre: &str, s: &str, out: &mut String) {
    let mut i = 0;
    while i < s.len() {
        let Some(open) = scan(s, i, b"{};") else { return };
        let head = s[i..open].trim();
        if s.as_bytes()[open] != b'{' {
            i = open + 1;
            continue;
        }
        let Some(close) = close_of(s, open) else { return };
        let body = &s[open + 1..close];
        if head.starts_with('@') {
            out.push_str(head);
            out.push_str(" {\n");
            if head.to_ascii_lowercase().contains("keyframes") {
                out.push_str(body.trim());
                out.push('\n');
            } else {
                scope_into(pre, body, out);
            }
            out.push_str("}\n");
        } else {
            let sels: Vec<String> = split_top(head, b',')
                .iter()
                .map(|sel| {
                    let sel = sel.trim();
                    match sel.strip_prefix(":scope") {
                        Some(rest) => format!("{pre}{rest}"),
                        None => format!("{pre} {sel}"),
                    }
                })
                .collect();
            out.push_str(&sels.join(", "));
            out.push_str(" { ");
            out.push_str(body.trim());
            out.push_str(" }\n");
        }
        i = close + 1;
    }
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;
