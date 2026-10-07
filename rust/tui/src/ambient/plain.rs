//! Main's markdown as the capsule shows it: plain text, ready to set
//! (ambient's review, m_4672). The marks go, the words stay: `**x**`,
//! `*x*`, `` `x` `` and `~~x~~` keep x, a link keeps its text, an image
//! its alt text, a heading or a quote loses its `#` / `>`, a `* ` or `+ `
//! item becomes `- `, a code fence's lines go (its code stays), a table's
//! separator row goes and its cells are joined by ` · `. Lines and blank
//! lines (the paragraphs) are kept; `_` is left alone (snake_case).

pub fn plain(md: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_code = false;
    for raw in md.lines() {
        let line = raw.trim_end();
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            out.push(line.to_string());
            continue;
        }
        if is_table_rule(t) {
            continue;
        }
        let indent = &line[..line.len() - t.len()];
        let body = block(t);
        out.push(format!("{indent}{}", inline(&body)));
    }
    // no blank runs, none at the ends
    let mut kept: Vec<String> = Vec::new();
    for l in out {
        if l.trim().is_empty() && kept.last().is_none_or(|p: &String| p.trim().is_empty()) {
            continue;
        }
        kept.push(l);
    }
    while kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    kept.join("\n")
}

/// `|---|:--:|` and the like.
fn is_table_rule(t: &str) -> bool {
    t.contains('-') && t.contains('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// The line's block mark: a heading, a quote, a list item, a table row.
fn block(t: &str) -> String {
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
        return t[hashes..].trim().to_string();
    }
    if let Some(r) = t.strip_prefix('>') {
        return block(r.trim_start());
    }
    if let Some(r) = t.strip_prefix("* ").or_else(|| t.strip_prefix("+ ")) {
        return format!("- {r}");
    }
    if t.starts_with('|') && t.ends_with('|') && t.len() > 1 {
        let cells: Vec<&str> = t[1..t.len() - 1].split('|').map(str::trim).collect();
        return cells.join(" · ");
    }
    t.to_string()
}

/// The inline marks of one line.
fn inline(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut o = String::with_capacity(s.len());
    let mut i = 0;
    while i < c.len() {
        // `code`: its text as is
        if c[i] == '`' {
            if let Some(j) = find(&c, i + 1, '`') {
                o.extend(&c[i + 1..j]);
                i = j + 1;
                continue;
            }
        }
        // ![alt](url) and [text](url)
        let img = c[i] == '!' && c.get(i + 1) == Some(&'[');
        if c[i] == '[' || img {
            let open = if img { i + 1 } else { i };
            if let Some(close) = find(&c, open + 1, ']') {
                if c.get(close + 1) == Some(&'(') {
                    if let Some(end) = find(&c, close + 2, ')') {
                        o.push_str(&inline(&c[open + 1..close].iter().collect::<String>()));
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        // **x**, ~~x~~
        for m in ["**", "~~"] {
            let mc: Vec<char> = m.chars().collect();
            if c[i..].starts_with(&mc) {
                if let Some(j) = find_seq(&c, i + 2, &mc) {
                    o.push_str(&inline(&c[i + 2..j].iter().collect::<String>()));
                    i = j + 2;
                    break;
                }
            }
        }
        if i < c.len() && c[i] == '*' {
            // *x*: an emphasis opens before a non-space and closes after one
            let opens = c.get(i + 1).is_some_and(|n| !n.is_whitespace() && *n != '*');
            if opens {
                if let Some(j) = find(&c, i + 1, '*').filter(|&j| !c[j - 1].is_whitespace()) {
                    o.push_str(&inline(&c[i + 1..j].iter().collect::<String>()));
                    i = j + 1;
                    continue;
                }
            }
        }
        if i < c.len() {
            o.push(c[i]);
            i += 1;
        }
    }
    o
}

fn find(c: &[char], from: usize, ch: char) -> Option<usize> {
    (from..c.len()).find(|&k| c[k] == ch)
}

fn find_seq(c: &[char], from: usize, seq: &[char]) -> Option<usize> {
    (from..c.len()).find(|&k| c[k..].starts_with(seq))
}

#[cfg(test)]
mod tests {
    use super::plain;

    #[test]
    fn marks_go_words_stay() {
        assert_eq!(plain("**done**: the `gate` is *green*, see [the PR](https://x/1)."), "done: the gate is green, see the PR.");
        assert_eq!(plain("## Result\n\n> all ~~red~~ green\n* one\n+ two\n- three"), "Result\n\nall red green\n- one\n- two\n- three");
        assert_eq!(plain("keep snake_case and 2 * 3 * 4"), "keep snake_case and 2 * 3 * 4");
        assert_eq!(plain("![the shot](a.png)"), "the shot");
    }

    #[test]
    fn code_and_tables() {
        assert_eq!(plain("run:\n\n```bash\ncargo test\n```\n"), "run:\n\ncargo test");
        assert_eq!(plain("| a | b |\n|---|:-:|\n| 1 | 2 |"), "a · b\n1 · 2");
    }

    #[test]
    fn blank_runs_fold() {
        assert_eq!(plain("\n\none\n\n\n\ntwo\n\n"), "one\n\ntwo");
    }
}
