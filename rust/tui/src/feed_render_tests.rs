//! Wire line -> feed rows, end to end: parsing, merging, code blocks,
//! wrapping and display widths.

use super::*;
use crate::code::*;
use crate::editor::layout_input;
use unicode_width::UnicodeWidthStr;

#[test]
fn provider_retry_and_restart_render() {
    match parse_line("  obs: provider_retry: 2/10 · provider 529 (transient) · retry in 4s") {
        Some(Ev::Warn(t)) => assert_eq!(
            t,
            "model call failed (attempt 2/10): provider 529 (transient) · retry 3/10 in 4s"
        ),
        _ => panic!("provider_retry must render as a warning"),
    }
    match parse_line("  obs: harness_restarted: exit status: 1 · bend: out of memory") {
        Some(Ev::Err(t)) => assert!(t.contains("bend: out of memory") && t.contains("restarted")),
        _ => panic!("harness_restarted must render as an error"),
    }
}

#[test]
fn split_thinking_takes_every_span() {
    let t = "<think>a\\nBENDSIG::s1</think>\\n<think>b\\nBENDSIG::s2</think>\\nok";
    let (think, vis) = split_thinking(t).unwrap();
    assert_eq!(think, "a\\nBENDSIG::s1\\nb\\nBENDSIG::s2");
    assert_eq!(vis, "ok");
    assert!(split_thinking("no markers").is_none());
    assert!(split_thinking("<think>open").is_none());
}

// the runtime's wire encoding, as emit_code_ann produces it
fn wire_encode(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\r', "\\R")
        .replace('\n', "\\N")
}

// proper JSON args, as the model emits them: quotes and backslashes
// escaped inside the code string, newlines as \n
fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

// the feed receives tool_started, the tool annotations, then
// tool_finished: the block must render under the merged line
#[test]
fn tool_code_block_renders_under_the_tool_line() {
    let code = "// orchestre les appels
async function main(): Promise<unknown> {
  const xs = [1, 2, 3];
  const s = \"une chaîne\";
  return xs.length + s.length;
}";
    let args = format!("{{\"code\": \"{}\"}}", json_escape(code));
    let wire_lines = vec![
        "  obs: tool_started #3".to_string(),
        format!("tool #3 run_typescript : {}", args),
        format!("tool_code #3 : {}", wire_encode(&args)),
        "  obs: tool_finished #3 ok".to_string(),
    ];
    let mut events: Vec<Ev> = Vec::new();
    let mut cache: Vec<Option<EventRows>> = Vec::new();
    for l in &wire_lines {
        let ev = parse_line(l).expect("parse");
        push_event(&mut events, &mut cache, ev);
    }
    let tool = events
        .iter()
        .find_map(|e| match e {
            Ev::Tool(td) => Some(td.clone()),
            _ => None,
        })
        .expect("the merged tool");
    assert_eq!(tool.id, 3);
    assert_eq!(tool.name.as_deref(), Some("run_typescript"));
    assert!(tool.code.is_some());
    let rows = ev_lines(&Ev::Tool(tool), 80);
    let joined: Vec<String> = rows
        .iter()
        .map(|r| r.spans.iter().map(|s| s.content.clone()).collect::<String>())
        .collect();
    for (i, l) in joined.iter().enumerate() {
        println!("{:2} | {}", i, l);
    }
    // the tool line: ƒ, the tool's name, its state; the source only in the block
    assert!(joined[0].starts_with("╭─ ƒ typescript ✓") && !joined[0].contains("orchestre"), "{}", joined[0]);
    assert!(joined.iter().any(|l| l.starts_with("│ async function main")));
    assert!(joined.iter().any(|l| l.contains("une chaîne")));
    // one row per source line, inside the box (BISE-96)
    assert_eq!(joined[1..].iter().take_while(|l| !l.starts_with('├') && !l.starts_with('╰')).count(), 6);
}

fn merged_tool(wire_lines: &[String]) -> ToolData {
    let mut events: Vec<Ev> = Vec::new();
    let mut cache: Vec<Option<EventRows>> = Vec::new();
    for l in wire_lines {
        let ev = parse_line(l).expect("parse");
        push_event(&mut events, &mut cache, ev);
    }
    events
        .iter()
        .find_map(|e| match e {
            Ev::Tool(td) => Some(td.clone()),
            _ => None,
        })
        .expect("the merged tool")
}

// BISE-223: the tool_intent annotation lands on its tool; an empty one
// is no intent
#[test]
fn the_intent_annotation_merges_into_its_tool() {
    let td = merged_tool(&[
        "  obs: tool_started #4".to_string(),
        "tool #4 bash : ls".to_string(),
        "tool_code #4 : ls".to_string(),
        "tool_intent #4 : je liste les fichiers".to_string(),
    ]);
    assert_eq!(td.intent.as_deref(), Some("je liste les fichiers"));
    assert_eq!(td.code.as_deref(), Some("ls"));
    assert!(parse_line("tool_intent #4 : ").is_none());
}

fn rows_text(rows: &[Line<'static>]) -> Vec<String> {
    rows.iter()
        .map(|r| r.spans.iter().map(|s| s.content.clone()).collect::<String>())
        .collect()
}

// BISE-11 then BISE-123: a long script is cut to the box's 15 inside
// rows while closed (`▸ n more lines` on the last one), whole once opened
#[test]
fn a_long_script_renders_whole_once_opened() {
    let cmd: String = (1..=200).map(|i| format!("echo {}", i)).collect::<Vec<_>>().join("\n");
    let bash = merged_tool(&[
        "  obs: tool_started #4".to_string(),
        "tool #4 bash : echo".to_string(),
        format!("tool_code #4 : {}", wire_encode(&cmd)),
        "  obs: tool_finished #4 ok".to_string(),
    ]);
    let closed = rows_text(&ev_lines(&Ev::Tool(bash.clone()), 80));
    assert_eq!(closed.iter().filter(|l| l.starts_with("│ echo")).count(), 14, "{closed:#?}");
    assert!(closed[closed.len() - 2].starts_with("│ ▸ 186 more lines"), "{closed:#?}");
    let mut bash = bash;
    bash.expanded = true;
    let rows = rows_text(&ev_lines(&Ev::Tool(bash), 80));
    assert_eq!(rows.iter().filter(|l| l.starts_with("│ echo")).count(), 200);
    assert!(rows.iter().any(|l| l.starts_with("│ echo 200 ")));
    assert!(!rows.iter().any(|l| l.contains("more lines")));
    // a TypeScript program too
    let src: String = (1..=200).map(|i| format!("const x{i} = {i};")).collect::<Vec<_>>().join("\n");
    let args = format!("{{\"code\": \"{}\"}}", json_escape(&src));
    let ts = merged_tool(&[
        "  obs: tool_started #5".to_string(),
        format!("tool #5 run_typescript : {}", args),
        format!("tool_code #5 : {}", wire_encode(&args)),
        "  obs: tool_finished #5 ok".to_string(),
    ]);
    let mut ts = ts;
    ts.expanded = true;
    let rows = rows_text(&ev_lines(&Ev::Tool(ts), 80));
    assert_eq!(rows.iter().filter(|l| l.starts_with("│ const x")).count(), 200);
}

// other code (a patch, however long) stays behind its ▸ until opened,
// then shows whole (BISE-12: the edit line is the fold)
#[test]
fn a_long_patch_folds_until_clicked() {
    let body: String = (1..=200).map(|i| format!("+line {}", i)).collect::<Vec<_>>().join("\n");
    let patch = format!("*** Begin Patch\n*** Add File: big.txt\n{}\n*** End Patch\n", body);
    let mut tool = merged_tool(&[
        "  obs: tool_started #6".to_string(),
        "tool #6 apply_patch : big".to_string(),
        format!("tool_code #6 : {}", wire_encode(&patch)),
        "  obs: tool_finished #6 ok".to_string(),
    ]);
    let folded = rows_text(&ev_lines(&Ev::Tool(tool.clone()), 80));
    assert_eq!(folded, vec![" ± edit big.txt ✓ +200 ▸".to_string()]);
    tool.expanded = true;
    let whole = rows_text(&ev_lines(&Ev::Tool(tool), 80));
    assert_eq!(whole.iter().filter(|l| l.contains("│ +line")).count(), 200);
}

#[test]
fn bash_code_block_renders_under_the_tool_line() {
    let cmd = "# compte les fichiers
for f in *.rs; do
  echo \"$f: $(wc -l < \"$f\")\"
done | sort -n";
    let wire_lines = vec![
        "  obs: tool_started #4".to_string(),
        format!("tool #4 bash : {}", cmd.replace('\n', " ")),
        format!("tool_code #4 : {}", wire_encode(cmd)),
        "  obs: tool_finished #4 ok".to_string(),
    ];
    let tool = merged_tool(&wire_lines);
    assert_eq!(tool.name.as_deref(), Some("bash"));
    let joined = rows_text(&ev_lines(&Ev::Tool(tool), 80));
    for (i, l) in joined.iter().enumerate() {
        println!("{:2} | {}", i, l);
    }
    // the tool line names the tool; the source shows only in the block
    assert!(joined[0].contains("bash") && !joined[0].contains("compte"));
    assert!(joined.iter().any(|l| l.contains("for f in *.rs; do")));
    assert!(joined.iter().any(|l| l.contains("done | sort -n")));
    // one row per source line (4 lines), inside the box (BISE-96)
    assert_eq!(joined[1..].iter().take_while(|l| !l.starts_with('├') && !l.starts_with('╰')).count(), 4);
}

// a bash tool line without tool_code (an older runtime) keeps the
// flattened args preview and renders no block
#[test]
fn bash_without_code_has_no_block() {
    let tool = merged_tool(&[
        "  obs: tool_started #5".to_string(),
        "tool #5 bash : ls -la".to_string(),
        "  obs: tool_finished #5 ok".to_string(),
    ]);
    let joined = rows_text(&ev_lines(&Ev::Tool(tool), 80));
    assert!(joined.iter().any(|l| l.contains("ls -la")));
    assert!(!joined.iter().any(|l| l.starts_with('├')), "no output, no rule");
}

fn style_of(lines: &[Vec<Span<'static>>], tok: &str) -> Style {
    lines
        .iter()
        .flatten()
        // adjacent same-style spans merge (whitespace included)
        .find(|s| s.content.trim() == tok)
        .map(|s| s.style)
        .unwrap_or_else(|| panic!("no span {:?}", tok))
}

// the lines LAWS.resume_replays_history pins on the Bend side: the
// client rebuilds the conversation, tools merged, no fake duration
#[test]
fn resume_history_rebuilds_the_feed() {
    let wire = [
        "history   obs: compaction_done: court...",
        "history you : salut\\nça va",
        "history   obs: tool_started #7",
        "history tool #7 bash : ls -la",
        "history tool_code #7 : ls -la",
        "history tool_result #7 fail : boom",
        "history   obs: tool_finished #7 failed",
        "history   obs: assistant: fini",
        "history   obs: tool_started #9",
        "history tool #9 bash : pwd",
        "history tool_code #9 : pwd",
        "history   obs: tool_finished #9 failed",
        "history injected : [notification] bg 0 done",
        "  obs: session_restored: 7 messages",
    ];
    let mut events: Vec<Ev> = Vec::new();
    let mut cache: Vec<Option<EventRows>> = Vec::new();
    for l in wire {
        let (line, replayed) = strip_history(l);
        let ev = if replayed { parse_history_line(line) } else { parse_line(line) };
        let Some(ev) = ev else { continue };
        let finished = match &ev {
            Ev::Tool(td) if !matches!(td.state, ToolState::Run) => Some(td.id),
            _ => None,
        };
        push_event(&mut events, &mut cache, ev);
        if let (true, Some(id)) = (replayed, finished) {
            hide_replayed_elapsed(&mut events, &mut cache, id);
        }
    }
    let kinds: Vec<String> = events
        .iter()
        .map(|e| match e {
            Ev::Compacted { text: t, .. } => format!("compacted {}", t),
            Ev::You(t, ..) => format!("you {}", t),
            Ev::Assistant(t) => format!("assistant {}", t),
            Ev::Tool(td) => format!(
                "tool {} {} ok={} code={} elapsed={:?}",
                td.id,
                td.name.clone().unwrap_or_default(),
                matches!(td.state, ToolState::Ok),
                td.code.is_some(),
                td.elapsed
            ),
            Ev::Info(t) => format!("info {}", t),
            _ => "other".to_string(),
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            "compacted court...".to_string(),
            "you salut\nça va".to_string(),
            "tool 7 bash ok=false code=true elapsed=Some(\"\")".to_string(),
            "assistant fini".to_string(),
            "tool 9 bash ok=false code=true elapsed=Some(\"\")".to_string(),
            "info injected · [notification] bg 0 done".to_string(),
            "info session restored · 7 messages".to_string(),
        ]
    );
}

// a long line wraps under the rail with a hanging indent and a faint
// »: nothing is lost, no row is wider than the block, the continuation
// rows join the row before when copied
#[test]
fn long_code_lines_wrap_with_a_hanging_indent() {
    let cmd = "cd /Users/someone/lab/project && cargo test -p some-crate --release 2>&1 | rg 'test result|FAILED|panicked' | head -20\necho ok";
    let lines = code_block_lines(cmd, CodeLang::Bash, &ToolState::Ok, 50);
    let rows = rows_text(&lines);
    for r in &rows {
        println!("{}", r);
    }
    assert!(rows.iter().all(|r| r.width() <= 50), "{rows:#?}");
    // 2 source lines, the first on several rows
    assert!(rows.len() > 3);
    assert!(rows[0].starts_with(" │ cd "));
    assert!(rows[1].starts_with(" │ » "), "{rows:#?}");
    assert!(crate::feedsel::is_soft(&lines[1]) && !crate::feedsel::is_soft(&lines[0]));
    assert_eq!(rows.last().unwrap(), " │ echo ok");
    // the text after the rail and the wrap marks reassembles the source
    let body: String = rows
        .iter()
        .map(|r| {
            let t = r.strip_prefix(" │ ").unwrap();
            t.strip_prefix("» ").unwrap_or(t).to_string()
        })
        .collect::<Vec<_>>()
        .join("");
    assert_eq!(body.replace(' ', ""), cmd.replace(['\n', ' '], ""));
    // the rail is a quiet line, the wrap mark faint
    assert_eq!(lines[1].spans[0].style.fg, Some(crate::theme::rule()));
    assert_eq!(lines[1].spans[1].style.fg, Some(crate::theme::faint()));
}

// a token longer than the row breaks hard, never overflows
#[test]
fn wrap_breaks_long_tokens() {
    let spans = vec![Span::raw("x".repeat(25))];
    let rows = wrap_code_line_hanging(&spans, 10, 10);
    let ws: Vec<usize> = rows.iter().map(|(_, w)| *w).collect();
    assert_eq!(ws, vec![10, 10, 5]);
    assert_eq!(wrap_code_line_hanging(&[], 10, 10).len(), 1);
    // continuation rows get their own, narrower width
    let ws: Vec<usize> = wrap_code_line_hanging(&spans, 10, 8).iter().map(|(_, w)| *w).collect();
    assert_eq!(ws, vec![10, 8, 7]);
}

// apply_patch renders as a diff: summary in the tool line, the
// envelope dropped, file headers, colored bands to the border
#[test]
fn apply_patch_renders_a_diff_block() {
    let patch = "*** Begin Patch
*** Update File: core/obs.bend
@@ def show_obs
   case T.Assistant{text}:
-    old line
+    new line
+    another line
*** Add File: notes.md
+# Notes
*** End Patch
";
    let tool = merged_tool(&[
        "  obs: tool_started #6".to_string(),
        format!("tool #6 apply_patch : {}", patch.replace('\n', " ")),
        format!("tool_code #6 : {}", wire_encode(patch)),
        "tool_result #6 ok : Done!".to_string(),
        "  obs: tool_finished #6 ok".to_string(),
    ]);
    let mut tool = tool;
    tool.expanded = true;
    let lines = ev_lines(&Ev::Tool(tool), 70);
    let rows = rows_text(&lines);
    for r in &rows {
        println!("{}", r);
    }
    assert_eq!(rows[0], " ± edit 2 files ✓ +3 −1 ▾");
    assert_eq!(patch_summary(&wire_decode(&wire_encode(patch))), "core/obs.bend +2 −1, notes.md +1");
    assert!(!rows.iter().any(|r| r.contains("Begin Patch") || r.contains("End Patch")));
    assert!(rows.iter().any(|r| r.starts_with(" │ ~ core/obs.bend")));
    assert!(rows.iter().any(|r| r.starts_with(" │ + notes.md · new")));
    // no line-number gutter in a diff
    assert!(rows.iter().any(|r| r.starts_with(" │ -    old line")));
}

#[test]
fn bash_highlighting_classifies_tokens() {
    let hl = highlight_bash(
        "# note
X=1 grep -n \"$HOME\" f.txt | wc -l && for i in 1 2; do echo $i; done",
    );
    let fg = |tok: &str| style_of(&hl, tok).fg;
    use crate::theme::{syntax_call, syntax_comment, syntax_keyword, syntax_number, syntax_type, text};
    assert_eq!(fg("# note"), Some(syntax_comment()));
    assert_eq!(fg("X"), Some(syntax_number())); // assignment name
    assert_eq!(fg("grep"), Some(syntax_call())); // command word
    assert_eq!(fg("-n"), Some(syntax_type())); // option
    assert_eq!(fg("$HOME"), Some(syntax_number())); // expansion in quotes
    assert_eq!(fg("f.txt"), Some(text())); // argument
    assert_eq!(fg("wc"), Some(syntax_call())); // command after a pipe
    assert_eq!(fg("for"), Some(syntax_keyword()));
    assert_eq!(fg("in"), Some(syntax_keyword()));
    assert_eq!(fg("do"), Some(syntax_keyword()));
    assert_eq!(fg("echo"), Some(syntax_call())); // command after "do"
    assert_eq!(fg("done"), Some(syntax_keyword()));
    assert_eq!(hl.len(), 2); // one span list per source line
}

// ---- display widths (emoji are 2 columns) ----

fn row_widths(input: &str, inner: usize) -> Vec<usize> {
    layout_input(input, inner)
        .iter()
        .map(|r| r.iter().filter(|c| !c.newline).map(|c| c.w).sum())
        .collect()
}

#[test]
fn composer_rows_count_emojis_as_two_columns() {
    // 5 emojis at 6 columns: 3 per row, the 4th never splits a row
    assert_eq!(row_widths("👏👏👏👏👏", 6), vec![6, 4]);
    // an emoji that does not fit the row end moves to the next row
    assert_eq!(row_widths("abcde👏", 6), vec![5, 2]);
    for inner in 2..12 {
        for input in ["a👏b👍🏽c❤️d👨‍👩‍👧e🇫🇷", "👏👏👏👏👏👏👏", "x y 👏👏 z\nq👏"] {
            assert!(row_widths(input, inner).iter().all(|&w| w <= inner), "{input} @ {inner}");
        }
    }
}

#[test]
fn composer_cells_are_graphemes_with_char_indices() {
    let rows = layout_input("a👍🏽❤️👨‍👩‍👧b", 40);
    let cells: Vec<(usize, &str, usize)> = rows[0].iter().map(|c| (c.ci, c.text, c.w)).collect();
    assert_eq!(
        cells,
        vec![(0, "a", 1), (1, "👍🏽", 2), (3, "❤️", 2), (5, "👨‍👩‍👧", 2), (10, "b", 1), (11, " ", 1)]
    );
    // the end cursor slot sits after the last char
    assert!(rows[0].last().unwrap().newline);
}

#[test]
fn feed_wrap_counts_graphemes_as_drawn() {
    let w = |l: &Line| l.spans.iter().map(|s| s.content.width()).sum::<usize>();
    let rows = wrap_line(Line::from("👨‍👩‍👧 👨‍👩‍👧 👨‍👩‍👧"), 8);
    assert_eq!(rows.len(), 1, "3 × 2 cols + 2 spaces fit 8");
    let rows = wrap_line(Line::from("👏👏👏👏👏"), 4);
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|r| w(r) <= 4));
    let rows = wrap_code_line_hanging(&[Span::raw("❤️❤️❤️")], 4, 4);
    assert_eq!(rows.iter().map(|r| r.1).collect::<Vec<_>>(), vec![4, 2]);
}

// ---- the measure (BISE-10, book §11) ----

fn feed_rows(events: &[Ev], width: usize) -> Vec<Vec<String>> {
    (0..events.len())
        .map(|i| rows_text(&build_rows(events, i, false, width, 0)))
        .collect()
}

fn measure_feed() -> Vec<Ev> {
    let prose = "the login breaks on safari because the session cookie is set with SameSite=None and without Secure, so the browser drops it on the redirect and the user lands on the sign-in page again. ".repeat(2);
    let long_line = format!("cargo test -p bend-tui --release -- {} --nocapture", "feed_render ".repeat(14));
    let tool = merged_tool(&[
        "  obs: tool_started #3".to_string(),
        "tool #3 bash : cargo".to_string(),
        format!("tool_code #3 : {}", wire_encode(&format!("cd rust\n{}", long_line))),
        "  obs: tool_finished #3 ok".to_string(),
    ]);
    // the script's measure: the call opened into its box
    let mut tool = tool;
    tool.opened = true;
    vec![
        Ev::You(prose.clone(), Mark::Read, false),
        Ev::Assistant(prose.clone()),
        Ev::AgentMsg { from: "docs".into(), to: "main".into(), text: prose.clone(), level: 3, id: String::new(), open: false, fold: false },
        Ev::Thinking { ms: 1200, text: prose.clone(), open: true },
        Ev::Tool(tool),
    ]
}

#[test]
fn prose_wraps_at_91_and_code_at_103() {
    // the reading column (book §8, BISE-101): 3 lead + 88 of text; code to 103
    let (pm, cm) = (crate::render::PROSE_MAX, crate::render::CODE_MAX);
    for width in [60usize, 100, 160] {
        let events = measure_feed();
        let rows = feed_rows(&events, width);
        let widest = |rs: &[String]| rs.iter().map(|r| r.trim_end().width()).max().unwrap_or(0);
        // prose: the user block, the reply, the reasoning
        for (i, rs) in rows[..4].iter().enumerate().filter(|(i, _)| *i != 2) {
            let w = widest(rs);
            assert!(w <= pm.min(width), "event {i} at {width}: {w} columns\n{rs:#?}");
            // the text fills its measure: it is not wrapped narrower
            assert!(w >= pm.min(width) - 12, "event {i} at {width}: only {w} columns\n{rs:#?}");
        }
        // a message between agents: a chip alone on its row, 2 rows of
        // text at most under it (BISE-127), the code measure
        let msg: Vec<&String> = rows[2].iter().filter(|r| !r.is_empty()).collect();
        assert!(msg.len() <= 3 && msg[1..].iter().all(|r| r.starts_with("  ")), "{width}: {msg:#?}");
        assert!(msg.iter().all(|r| r.trim_end().width() <= cm.min(width)), "{width}: {msg:#?}");
        // the user block's panel stops at the measure too (a wrapped
        // row keeps the blank after its last word, as before)
        assert!(rows[0].iter().all(|r| r.trim_end().width() <= pm.min(width) && r.width() <= (pm + 1).min(width + 1)), "{width}: {:#?}", rows[0]);
        // code: the tool line and its script
        let code = &rows[4];
        let w = widest(code);
        assert!(w <= cm.min(width), "code at {width}: {w}\n{code:#?}");
        if width > cm {
            // the long script line uses the whole code measure
            assert!(w >= 90, "code at {width}: {w}\n{code:#?}");
        }
        assert!(code.iter().any(|r| r.starts_with("│ » ")), "code at {width}: {code:#?}");
    }
}

// ---- progressive disclosure (BISE-12, book §11) ----

fn tool_with(id: u32, name: &str, code: Option<&str>, result: Option<(bool, &str)>, ok: bool) -> ToolData {
    let mut lines = vec![format!("  obs: tool_started #{id}"), format!("tool #{id} {name} : x")];
    if let Some(c) = code {
        lines.push(format!("tool_code #{id} : {}", wire_encode(c)));
    }
    if let Some((rok, r)) = result {
        lines.push(format!("tool_result #{id} {} : {r}", if rok { "ok" } else { "fail" }));
    }
    lines.push(format!("  obs: tool_finished #{id} {}", if ok { "ok" } else { "failed" }));
    merged_tool(&lines)
}

fn text_of(ev: &Ev, width: usize) -> Vec<String> {
    rows_text(&ev_rows(ev, 0, width))
}

#[test]
fn an_output_is_one_line_until_opened() {
    // bash / TypeScript: the output is inside the box, whole when short
    let out = "running 12 tests  test login ... FAILED  1 failed, 11 passed (6.1s)";
    let td = tool_with(3, "bash", Some("npx playwright test"), Some((true, out)), true);
    let closed = text_of(&Ev::Tool(td), 100);
    assert!(closed.iter().any(|r| r.starts_with("│ npx playwright test ")), "{closed:#?}");
    assert!(closed.iter().any(|r| r.starts_with('├')), "{closed:#?}");
    assert!(closed.iter().any(|r| r.contains("11 passed")), "{closed:#?}");
    assert!(closed.last().unwrap().starts_with('╰'), "{closed:#?}");
    // other tools: one line until opened
    let mut td = tool_with(4, "search", None, Some((true, "3 results\nmore")), true);
    assert_eq!(text_of(&Ev::Tool(td.clone()), 100).last().unwrap(), "   ▸ output");
    td.expanded = true;
    let open = text_of(&Ev::Tool(td), 100);
    assert!(open.iter().any(|r| r == "   ▾ output"), "{open:#?}");
}

#[test]
fn a_failure_is_one_line_with_its_reason() {
    // a failing bash call is a box with an error border (BISE-96)
    let reason = "error[E0425]: cannot find value `width` in this scope";
    let td = tool_with(5, "bash", Some("cargo test"), Some((false, reason)), false);
    let rows = text_of(&Ev::Tool(td), 100);
    assert!(rows[0].starts_with("╭─ $ bash ✗"), "{rows:#?}");
    assert!(rows.iter().any(|r| r.starts_with(&format!("│ {reason}"))), "{rows:#?}");
    // another tool: one line, its reason first; `▸` when cut
    let long = format!("{reason}  {}", "--> tui/src/feed.rs:12:5 ".repeat(8));
    let mut td = tool_with(6, "read", None, Some((false, &long)), false);
    let closed = text_of(&Ev::Tool(td.clone()), 100);
    let last = closed.last().unwrap();
    assert!(last.starts_with(&format!("   {reason}")) && last.ends_with("… ▸"), "{closed:#?}");
    td.expanded = true;
    let open = text_of(&Ev::Tool(td), 100);
    assert!(open.iter().any(|r| r.contains("feed.rs:12:5")));
    let td = tool_with(7, "read", None, Some((false, "exit 1")), false);
    assert_eq!(text_of(&Ev::Tool(td), 100).last().unwrap(), "   exit 1");
}

#[test]
fn an_edit_is_one_line_its_diff_when_opened() {
    let patch = "*** Begin Patch\n*** Update File: web/src/auth/session.ts\n@@\n   cookie: {\n-    sameSite: \"none\",\n+    sameSite: \"none\",\n+    secure: true,\n*** End Patch\n";
    let mut td = tool_with(7, "apply_patch", Some(patch), Some((true, "Done!")), true);
    let closed = text_of(&Ev::Tool(td.clone()), 100);
    assert_eq!(closed, vec![" ± edit web/src/auth/session.ts ✓ +2 −1 ▸".to_string()]);
    td.expanded = true;
    let open = text_of(&Ev::Tool(td), 100);
    assert_eq!(open[0], " ± edit web/src/auth/session.ts ✓ +2 −1 ▾");
    assert!(open.iter().any(|r| r == " │ +    secure: true,"), "{open:#?}");
    // several files; a failure gives its reason
    let two = "*** Begin Patch\n*** Update File: a.rs\n+x\n*** Add File: b.rs\n+y\n*** End Patch\n";
    let td = tool_with(8, "apply_patch", Some(two), Some((false, "the patch doesn't apply: context not found")), false);
    assert_eq!(
        text_of(&Ev::Tool(td), 100),
        vec![" ± edit 2 files ✗ the patch doesn't apply: context not found ▸".to_string()]
    );
}

// approvals-edit: Vibe's edit and write_file come as the V4A patch they
// amount to (bend/core/edit.bend edit_view / write_view): the same diff
#[test]
fn vibe_edit_and_write_file_show_as_diffs() {
    let patch = "*** Begin Patch\n*** Update File: src/a.rs\n@@\n-old\n+new\n+more\n*** End Patch";
    let mut td = tool_with(9, "edit", Some(patch), Some((true, "file: /w/src/a.rs")), true);
    assert_eq!(text_of(&Ev::Tool(td.clone()), 100), vec![" ± edit src/a.rs ✓ +2 −1 ▸".to_string()]);
    td.expanded = true;
    assert!(text_of(&Ev::Tool(td), 100).iter().any(|r| r == " │ +more"));
    let add = "*** Begin Patch\n*** Add File: notes/n.md\n+one\n+two\n*** End Patch";
    let td = tool_with(10, "write_file", Some(add), Some((true, "file_path: /w/notes/n.md")), true);
    assert_eq!(text_of(&Ev::Tool(td), 100), vec![" ± write notes/n.md ✓ +2 ▸".to_string()]);
}

fn agent_msg(from: &str, text: &str) -> Ev {
    Ev::AgentMsg { from: from.into(), to: String::new(), text: text.into(), level: 3, id: "m_3".into(), open: false, fold: false }
}

#[test]
fn a_report_is_one_line_until_opened() {
    let mut ev = agent_msg("bench", "[report: done] p95 at 180 ms, nothing to fix.\n- ran 3 times on staging\n- p99 410 ms");
    let closed: Vec<String> = text_of(&ev, 100).into_iter().filter(|r| !r.is_empty()).collect();
    assert_eq!(closed, vec![" ✓ bench: p95 at 180 ms, nothing to fix. ▸ report".to_string()]);
    let mut cache: Vec<Option<EventRows>> = vec![None];
    assert!(toggle_event(std::slice::from_mut(&mut ev), &mut cache, 0));
    let open: Vec<String> = text_of(&ev, 100).into_iter().filter(|r| !r.is_empty()).collect();
    assert_eq!(open[0], " ✓ bench: p95 at 180 ms, nothing to fix. ▾ report");
    assert!(open.iter().any(|r| r.starts_with(" │ ") && r.contains("p99 410 ms")), "{open:#?}");
    // the kind gives the glyph; a one-line report has no ▸
    let failed = text_of(&agent_msg("deploy", "[report: failed] the staging token expired."), 100);
    assert!(failed.iter().any(|r| r == " ✗ deploy: the staging token expired."), "{failed:#?}");
    let blocked = text_of(&agent_msg("docs", "[report: blocked] it needs the v2 schema file."), 100);
    assert!(blocked.iter().any(|r| r.starts_with(" ? docs: ")), "{blocked:#?}");
    // a long summary is cut to one line, the ▸ shows the rest
    let long = text_of(&agent_msg("docs", &format!("[report: done] {}", "word ".repeat(40))), 76);
    let row = long.iter().find(|r| !r.is_empty()).unwrap();
    assert!(row.ends_with("… ▸ report") && row.width() <= 76, "{long:#?}");
}

#[test]
fn the_brief_is_one_line_until_opened() {
    let mut ev = agent_msg("main", "# Task `auth-fix`\n\nObjective: the login breaks on safari 18.");
    assert_eq!(text_of(&ev, 100), vec![" ◇ brief ▸".to_string()]);
    let mut cache: Vec<Option<EventRows>> = vec![None];
    toggle_event(std::slice::from_mut(&mut ev), &mut cache, 0);
    let open = text_of(&ev, 100);
    assert_eq!(open[0], " ◇ brief ▾");
    assert!(open.iter().any(|r| r == " │ Objective: the login breaks on safari 18."), "{open:#?}");
    assert!(!open.iter().any(|r| r.contains("# Task")));
}

#[test]
fn toggles_one_item() {
    let mut app = crate::sb::bench::test_app();
    // one-line tools (a bash box folds only past 15 output rows)
    let out = tool_with(1, "search", None, Some((true, "a b c")), true);
    let quiet = tool_with(2, "search", None, None, true);
    app.events = vec![
        Ev::Tool(out.clone()),
        Ev::Thinking { ms: 10, text: "hm".into(), open: false },
        Ev::Tool(out),
        Ev::Tool(quiet),
        Ev::Info("x".into()),
    ];
    app.cache = (0..5).map(|_| None).collect();
    let expanded = |app: &App| -> Vec<bool> {
        app.events.iter().filter_map(|e| match e { Ev::Tool(td) => Some(td.expanded), _ => None }).collect()
    };
    // no selection: nothing to toggle
    assert!(!toggle_selected(&mut app));
    // the selection on the thinking section opens it
    app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (1, 0, 0), head: (1, 0, 3) });
    assert!(toggle_selected(&mut app));
    assert!(matches!(app.events[1], Ev::Thinking { open: true, .. }));
    // on an output
    app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (0, 0, 0), head: (0, 0, 0) });
    ensure_rows(&app.events, &mut app.cache, 0, false, 80, 0);
    assert!(toggle_selected(&mut app));
    assert_eq!(expanded(&app), vec![true, false, false]);
    assert!(app.cache[0].is_none(), "its rows rebuild");
    // nothing behind a notice or a tool without output
    app.feed_sel = Some(crate::feedsel::FeedSel { anchor: (4, 0, 0), head: (4, 0, 0) });
    assert!(!toggle_selected(&mut app));
    assert!(!toggle_event(&mut app.events, &mut app.cache, 3));
    // thinking is ctrl+o's, untouched
    assert!(matches!(app.events[1], Ev::Thinking { open: true, .. }));
}

#[test]
fn fit_chars_keeps_the_ellipsis_inside() {
    assert_eq!(crate::render::fit_chars("abcdef", 6), "abcdef");
    assert_eq!(crate::render::fit_chars("abcdefg", 6), "abcde…");
}

// ---- the mockups (BISE-13): tui-screens.html "inside an agent" and
// "everything disclosed", as feed rows ----

fn mockup_turn(open: bool) -> Vec<Ev> {
    let bash = "cd web\nnpx playwright test login --project=webkit --reporter=line\ngrep -rn \"SameSite\" src/auth/";
    let ts = "async function main() {\n  const issues = await tools.github.search_issues({\n    query: \"safari login cookie\",\n    limit: 5,\n  });\n  // only the open ones, newest first\n  return issues.filter((i) => i.state === \"open\").map((i) => i.title);\n}";
    let args = format!("{{\"code\": \"{}\"}}", json_escape(ts));
    let out = "[webkit] › login.spec.ts:12 › logs in and stays logged in  expected cookie \"sid\" to be set after redirect  1 failed, 11 passed (6.1s)";
    let patch = "*** Begin Patch\n*** Update File: web/src/auth/session.ts\n@@ -29,5 +29,7 @@\n   cookie: {\n-    sameSite: \"none\",\n+    sameSite: \"none\",\n+    secure: true,\n   },\n*** End Patch\n";
    let mut b = tool_with(1, "bash", Some(bash), Some((true, out)), true);
    let mut t = merged_tool(&[
        "  obs: tool_started #2".to_string(),
        format!("tool #2 run_typescript : {}", args),
        format!("tool_code #2 : {}", wire_encode(&args)),
        "tool_result #2 ok : [\"Safari drops the session cookie\", \"Login loop on webkit\"]".to_string(),
        "  obs: tool_finished #2 ok".to_string(),
    ]);
    let mut p = tool_with(3, "apply_patch", Some(patch), Some((true, "Done!")), true);
    for td in [&mut b, &mut t, &mut p] {
        td.elapsed = Some(String::new());
        // ctrl+o opens a call's row into its box (BISE-223)
        (td.expanded, td.opened) = (open, open);
    }
    let brief = "# Task `auth-fix`\n\nthe login breaks on safari 18. reproduce with playwright, fix it,\nkeep the chrome path unchanged. report with the test output.";
    vec![
        Ev::AgentMsg { from: "main".into(), to: String::new(), text: brief.into(), level: 3, id: "m_1".into(), open, fold: false },
        Ev::Thinking { ms: 14_000, text: "safari drops the session cookie on the redirect. SameSite=None needs\nSecure, and the dev server sets it without. check session.ts first.".into(), open },
        Ev::Tool(b),
        Ev::Tool(t),
        Ev::Sub { name: "github.search_issues".into(), ok: true, preview: "2 items".into() },
        Ev::Tool(p),
        Ev::Assistant("found it: safari drops SameSite=None cookies without Secure. added secure: true; the webkit test passes now.".into()),
    ]
}

fn feed_text(events: &[Ev], width: usize) -> Vec<String> {
    feed_rows(events, width).into_iter().flatten().map(|r| r.trim_end().to_string()).collect()
}

/// A box row without its padding and right border (BISE-96), so the
/// mockup's rows compare by their text.
fn unbox(r: &str) -> String {
    r.trim_end_matches([' ', '│', '─', '╮', '╯', '┤']).to_string()
}

#[test]
fn inside_an_agent_matches_the_mockup() {
    // a call's row pads its state to the right edge: one space here
    let squash = |r: &str| {
        let mut r = r.to_string();
        while r.contains("  ") {
            r = r.replace("  ", " ");
        }
        r
    };
    let rows: Vec<String> = feed_text(&mockup_turn(false), 100).iter().map(|r| squash(&unbox(r))).collect();
    println!("{}", rows.join("\n"));
    let want = [
        " ◇ brief ▸",
        " ∴ thought for 14s ▸",
        // BISE-223: an agent's calls are rows too (no description: the
        // script's first line)
        " $ cd web ✓",
        " ƒ async function main() { ✓",
        " ± edit web/src/auth/session.ts ✓ +2 −1 ▸",
        " found it: safari drops SameSite=None cookies without Secure. added secure: true; the",
    ];
    for w in want {
        assert!(rows.iter().any(|r| r == w), "{w:?} missing:\n{}", rows.join("\n"));
    }
    // in that order
    let at = |w: &str| rows.iter().position(|r| r == w).unwrap();
    assert!(want.windows(2).all(|p| at(p[0]) < at(p[1])));
}

#[test]
fn everything_disclosed_matches_the_mockup() {
    let rows: Vec<String> = feed_text(&mockup_turn(true), 100).iter().map(|r| unbox(r)).collect();
    println!("{}", rows.join("\n"));
    let want = [
        " ◇ brief ▾",
        " │ the login breaks on safari 18. reproduce with playwright, fix it,",
        " ∴ thought for 14s ▾",
        " │ Secure, and the dev server sets it without. check session.ts first.",
        "╭─ $ bash ✓",
        " ± edit web/src/auth/session.ts ✓ +2 −1 ▾",
        " │ +    secure: true,",
    ];
    for w in want {
        assert!(rows.iter().any(|r| r == w), "{w:?} missing:\n{}", rows.join("\n"));
    }
    assert!(rows.iter().any(|r| r.starts_with("│ [webkit] › login.spec.ts:12")));
}

// a card in the history: a question or a blocker is level 1 (accent bar,
// bold accent `? {name} needs you`, the body in text); done is one line
#[test]
fn a_card_is_level_one() {
    let lines = ev_rows(&Ev::Card { text: "#2 question @docs : v1 or v2 for the api docs?".into(), closed: String::new() }, 0, 100);
    let rows = rows_text(&lines);
    assert_eq!(rows, vec![" ┃ ? docs needs you".to_string(), " ┃ v1 or v2 for the api docs?".to_string()]);
    let accent = Some(crate::theme::accent());
    assert_eq!(lines[0].spans[0].style.fg, accent);
    assert_eq!(lines[0].spans[1].style.fg, accent);
    assert!(lines[0].spans[1].style.add_modifier.contains(Modifier::BOLD));
    assert_eq!(lines[1].spans[1].style.fg, Some(crate::theme::text()));
    let blocked = rows_text(&ev_rows(&Ev::Card { text: "#4 blocked @api-v2 : the schema file isn't in the repo".into(), closed: String::new() }, 0, 100));
    assert_eq!(blocked[0], " ┃ ? api-v2 needs you");
    let done = rows_text(&ev_rows(&Ev::Card { text: "#3 done @bench : p95 at 180 ms".into(), closed: String::new() }, 0, 100));
    assert_eq!(done, vec![" ✓ bench is done: p95 at 180 ms".to_string()]);
}

// BISE-31 (book §10, §12): an answered card fades in place — dim bar,
// dim title with `· answered`, dim body — and nothing is appended; the
// answer is the hub's own line after it. Other results in plain words;
// a card not in the feed (an older page) closes as an info line.
#[test]
fn an_answered_card_fades_in_place() {
    let mut events: Vec<Ev> = Vec::new();
    let mut cache: Vec<Option<EventRows>> = Vec::new();
    let card = |t: &str| Ev::Card { text: t.into(), closed: String::new() };
    push_event(&mut events, &mut cache, card("#2 question @docs : v1 or v2 for the api docs?"));
    push_event(&mut events, &mut cache, card("#3 blocked @api : the schema file"));
    push_event(&mut events, &mut cache, Ev::Info("→ you → @docs (answer to card #2) : v2".into()));
    assert!(!push_event(&mut events, &mut cache, Ev::CardClosed { id: 2, res: "answered".into() }));
    assert_eq!(events.len(), 3, "nothing appended");
    let lines = ev_rows(&events[0], 0, 100);
    assert_eq!(
        rows_text(&lines),
        vec![" ┃ ? docs needs you · answered".to_string(), " ┃ v1 or v2 for the api docs?".to_string()]
    );
    let dim = Some(crate::theme::dim());
    // bar, glyph and title all dim (one span), not bold
    assert!(lines[0].spans.iter().all(|s| s.style.fg == dim), "title");
    assert!(lines[0].spans.iter().all(|s| !s.style.add_modifier.contains(Modifier::BOLD)));
    // the bar and the body, both dim, are one span
    assert!(lines[1].spans.iter().all(|s| s.style.fg == dim), "body");
    // the other card is untouched
    assert_eq!(rows_text(&ev_rows(&events[1], 0, 100))[0], " ┃ ? api needs you");
    // the hub's words, for the user
    for (res, word) in [
        ("answered via @main", "answered by main"),
        ("closed", "closed"),
        ("accepted", "dropped"),
        ("refused", "kept"),
        ("task stopped", "agent stopped"),
    ] {
        let ev = Ev::Card { text: "#5 drop @x : y".into(), closed: res.into() };
        assert_eq!(rows_text(&ev_rows(&ev, 0, 100))[0], format!(" ┃ ? x needs you · {}", word));
    }
    // a done card is a line for you: it does not fade
    push_event(&mut events, &mut cache, card("#4 done @bench : p95 at 180 ms"));
    assert!(!push_event(&mut events, &mut cache, Ev::CardClosed { id: 4, res: "vue".into() }));
    assert_eq!(rows_text(&ev_rows(events.last().unwrap(), 0, 100)), vec![" ✓ bench is done: p95 at 180 ms".to_string()]);
    // not in the feed (an older page, or a done card that became its
    // report line, BISE-90): nothing is appended
    let n = events.len();
    assert!(!push_event(&mut events, &mut cache, Ev::CardClosed { id: 9, res: "answered".into() }));
    assert_eq!(events.len(), n);
}

// the other §6 entities of the feed
/// computer use (design §8, designer m_3774): each action of a code-mode
/// run is its own row under the run, `↖ <summary>`; a failure `✗` in the
/// error color, a pause or stop dim without `✗`; quiet calls (snapshot)
/// stay inside the box. CU_DUMP=<file>: the rows for designer.
#[test]
fn computer_actions_are_rows_of_their_own() {
    let mut t = tool_with(1, "run_typescript", None, Some((true, "done")), true);
    t.elapsed = Some(String::new());
    let sub = |name: &str, ok: bool, preview: &str| Ev::Sub { name: name.into(), ok, preview: preview.into() };
    let events = vec![
        Ev::Tool(t),
        sub("computer.snapshot", true, r##"{"refs":12,"target":"tab:7","text":"# Anker cable · amazon.fr"##),
        sub("computer.act", true, r##"{"summary":"clicked \"Add to cart\" · amazon.fr","changed":"- button \"Added\" [e4]","ok":true"##),
        sub("computer.act", true, r##"{"summary":"typed in the search box · Figma","changed":"","ok":true,"title":"Figma"}"##),
        sub("computer.act", true, r##"{"error":{"summary":"couldn't click \"Add to cart\": a popup covers it · amazon.fr","candidates":[],"code":"not_found""##),
        sub("computer.act", true, r##"{"error":{"summary":"paused · you took the wheel in Mail","code":"paused","message":"m"}}"##),
    ];
    let rows: Vec<String> = feed_text(&events, 90).iter().map(|r| r.trim_end().to_string()).filter(|r| !r.is_empty()).collect();
    if let Some(f) = std::env::var_os("CU_DUMP") {
        let _ = std::fs::write(f, rows.join("\n") + "\n");
    }
    let has = |w: &str| assert!(rows.iter().any(|r| r == w), "{w:?} in\n{}", rows.join("\n"));
    has(" ↖ clicked \"Add to cart\" · amazon.fr");
    has(" ↖ typed in the search box · Figma");
    has(" ✗ couldn't click \"Add to cart\": a popup covers it · amazon.fr");
    has("   paused · you took the wheel in Mail");
    assert!(!rows.iter().any(|r| r.contains("computer.act")), "{}", rows.join("\n"));
}

#[test]
fn feed_entities_use_the_book_glyphs() {
    let row = |ev: Ev| rows_text(&ev_rows(&ev, 0, 100)).join("\n");
    assert_eq!(row(Ev::Thinking { ms: 0, text: String::new(), open: false }), " ∴ thought ▸");
    assert_eq!(row(Ev::Compact), format!(" {} compacting", crate::theme::G_COMPACTING));
    assert_eq!(row(Ev::Compacted { text: "short".into(), open: false }), " ≡ summary ▸");
    assert_eq!(row(Ev::Compacted { text: "short".into(), open: true }), " ≡ summary ▾\n │ short");
    assert_eq!(row(Ev::Warn("turn interrupted".into())), " ▲ turn interrupted");
    assert_eq!(row(Ev::Warn("turn interrupted by main".into())), " ▲ turn interrupted by main");
    assert_eq!(row(Ev::Err("boom".into())), " ✗ boom");
    assert_eq!(row(Ev::Sub { name: "gh.x".into(), ok: false, preview: "404".into() }), "   ↳ gh.x ✗ 404");
    assert!(row(Ev::You("hi".into(), Mark::Read, false)).starts_with("│  hi"));
    // a tool the turn abandoned says so in English
    let mut events = vec![Ev::Tool(ToolData::bare(9, ToolState::Run))];
    let mut cache = vec![None];
    push_event(&mut events, &mut cache, Ev::TurnDone);
    assert!(matches!(&events[0], Ev::Tool(td) if td.result.as_ref().is_some_and(|r| r.1 == "interrupted")));
}

// ---- BISE-14: the three levels, folding, time marks (book §9, §10) ----

fn l3(from: &str, to: &str, text: &str) -> Ev {
    Ev::AgentMsg { from: from.into(), to: to.into(), text: text.into(), level: 3, id: String::new(), open: false, fold: false }
}

fn to_you(from: &str, text: &str) -> Ev {
    Ev::AgentMsg { from: from.into(), to: "you".into(), text: text.into(), level: 2, id: String::new(), open: false, fold: false }
}

/// Events pushed one by one, as they arrive (the merge and the cache
/// rules of push_event apply).
fn arrive(evs: Vec<Ev>) -> (Vec<Ev>, Vec<Option<EventRows>>) {
    let (mut events, mut cache) = (Vec::new(), Vec::new());
    for ev in evs {
        push_event(&mut events, &mut cache, ev);
    }
    (events, cache)
}

/// The rows through the cache, as a frame draws them.
fn cached_text(events: &[Ev], cache: &mut Vec<Option<EventRows>>, width: usize) -> Vec<String> {
    (0..events.len())
        .flat_map(|i| {
            ensure_rows(events, cache, i, false, width, 0);
            rows_text(&cache[i].as_ref().unwrap().rows)
        })
        .map(|r| r.trim_end().to_string())
        .collect()
}

/// A level-3 message drawn beside its chip (BISE-106), as `cached_text`
/// gives it: at x0, ` ✉︎ from → to `, 1 space, the text.
fn chip_row(from: &str, to: &str) -> String {
    format!(" {} {} → {}", crate::render::G_ENVELOPE, from, to)
}

/// A level-3 text under its chip, at x0+2 (BISE-127).
fn under(text: &str) -> String {
    format!("  {}", text)
}

/// `n` level-3 lines among `k` agents, numbered from `from`.
fn traffic(n: usize, k: usize, from: usize) -> Vec<Ev> {
    (0..n)
        .map(|j| {
            let m = from + j;
            l3(&format!("a{}", m % k), &format!("a{}", (m + 1) % k), &format!("message {}", m))
        })
        .collect()
}

#[test]
fn a_run_of_twelve_folds() {
    let mut evs = vec![Ev::You("ship it".into(), Mark::Read, false)];
    evs.extend(traffic(12, 5, 0));
    let (mut events, mut cache) = arrive(evs);
    let rows = cached_text(&events, &mut cache, 100);
    let pulse = crate::theme::working_frame(0).0;
    assert_eq!(
        rows,
        vec!["│  ship it ✓✓".to_string(), String::new(), format!("▸ 12 messages between 5 agents {}", pulse)],
        "the whole run is one line, live"
    );
    // opened in place, in order, each line a chip; every line is a new
    // pair here, so a blank row between them (BISE-106)
    assert!(toggle_event(&mut events, &mut cache, 1));
    let rows = cached_text(&events, &mut cache, 100);
    assert_eq!(rows[2], format!("▾ 12 messages between 5 agents {}", pulse));
    assert_eq!(rows.len(), 3 + 12 * 2 + 11, "{rows:#?}");
    for (j, r) in rows[3..].iter().enumerate() {
        let k = j / 3;
        let want = match j % 3 {
            0 => chip_row(&format!("a{}", k % 5), &format!("a{}", (k + 1) % 5)),
            1 => under(&format!("message {}", k)),
            _ => String::new(),
        };
        assert_eq!(*r, want);
    }
    // three lines or fewer never fold
    let (events, mut cache) = arrive(traffic(3, 5, 0));
    assert_eq!(cached_text(&events, &mut cache, 100).iter().filter(|r| r.contains('→')).count(), 3);
}

#[test]
fn a_level_two_line_closes_the_run() {
    let mut evs = traffic(5, 3, 0);
    evs.push(to_you("docs", "the v2 docs are up."));
    evs.extend(traffic(2, 3, 5));
    let (mut events, mut cache) = arrive(evs);
    let rows = cached_text(&events, &mut cache, 100);
    assert_eq!(rows[0], "▸ 5 messages between 3 agents", "closed: no pulse");
    assert_eq!(rows[1], "");
    assert_eq!(rows[2], format!(" {} docs to you: the v2 docs are up.", crate::theme::G_MSG));
    assert_eq!(rows[3], "");
    // a2 → a0 then a0 → a1: two pairs, a blank row between
    assert_eq!(rows[4..9], [chip_row("a2", "a0"), under("message 5"), String::new(), chip_row("a0", "a1"), under("message 6")], "{rows:#?}");
    // the new run grows past three: it folds; the closed one stays
    for ev in traffic(2, 3, 7) {
        push_event(&mut events, &mut cache, ev);
    }
    let rows = cached_text(&events, &mut cache, 100);
    let pulse = crate::theme::working_frame(0).0;
    assert_eq!(rows[0], "▸ 5 messages between 3 agents");
    assert_eq!(rows[4], format!("▸ 4 messages between 3 agents {}", pulse));
    assert_eq!(rows.len(), 5, "{rows:#?}");
    // a main line (level 2) and a card (level 1) close it too
    for closer in [Ev::Assistant("done.".into()), Ev::Card { text: "#1 question @docs : v1 or v2?".into(), closed: String::new() }] {
        let mut evs = traffic(4, 3, 0);
        evs.push(closer);
        let (events, mut cache) = arrive(evs);
        assert_eq!(cached_text(&events, &mut cache, 100)[0], "▸ 4 messages between 3 agents");
    }
}

#[test]
fn a_closed_run_never_changes() {
    let mut evs = vec![Ev::You("go".into(), Mark::Read, false)];
    evs.extend(traffic(6, 4, 0));
    evs.push(Ev::Assistant("the agents agreed.".into()));
    let (mut events, mut cache) = arrive(evs);
    let before = cached_text(&events, &mut cache, 100);
    let n0 = events.len();
    // everything else arrives after it: more traffic, lines for you, tools
    let mut more = traffic(9, 7, 6);
    more.push(to_you("docs", "done"));
    more.push(Ev::Tool(tool_with(1, "bash", Some("ls"), Some((true, "a")), true)));
    more.extend(traffic(5, 2, 20));
    for ev in more {
        push_event(&mut events, &mut cache, ev);
    }
    let after = cached_text(&events, &mut cache, 100);
    assert_eq!(after[..before.len()], before[..], "cached");
    let fresh = feed_text(&events[..], 100);
    assert_eq!(fresh[..before.len()], before[..], "rebuilt");
    let rebuilt: Vec<String> = feed_text(&events[..n0], 100);
    assert_eq!(rebuilt, before);
}

#[test]
fn arrival_order_holds_with_many_agents() {
    // 30 agents talk over each other, lines for you in between: nothing
    // is grouped by agent, nothing moves
    let names: Vec<String> = (0..30).map(|k| format!("ep-{}", k)).collect();
    let mut evs = Vec::new();
    let mut want = Vec::new();
    for m in 0..120 {
        if m % 40 == 39 {
            evs.push(to_you(&names[m % 30], &format!("for you {}", m)));
            want.push(format!("for you {}", m));
        } else {
            evs.push(l3(&names[(m * 7) % 30], &names[(m * 11 + 3) % 30], &format!("line {}", m)));
            want.push(format!("line {}", m));
        }
    }
    let (mut events, mut cache) = arrive(evs);
    set_everything(&mut events, &mut cache, true);
    let rows = cached_text(&events, &mut cache, 100);
    let got: Vec<String> = rows
        .iter()
        .filter_map(|r| r.find("line ").or_else(|| r.find("for you ")).map(|k| r[k..].to_string()))
        .collect();
    assert_eq!(got, want);
    // the folds say how many lines and agents each run holds
    let folds: Vec<&String> = rows.iter().filter(|r| r.contains("messages between")).collect();
    assert_eq!(folds.len(), 3, "{folds:#?}");
    assert!(folds[0].contains("▾ 39 messages between 30 agents"), "{folds:#?}");
    // closed again: one line per run
    set_everything(&mut events, &mut cache, false);
    assert!(!anything_closed(&[]));
    assert!(anything_closed(&events));
    let rows = cached_text(&events, &mut cache, 100);
    assert_eq!(rows.iter().filter(|r| r.contains("line ")).count(), 0, "{rows:#?}");
}

#[test]
fn time_marks_after_a_pause() {
    let (mut events, mut cache) = arrive(vec![]);
    let at = || "14:31".to_string();
    // not at the top of a feed
    assert!(!pause_mark(&mut events, &mut cache, PAUSE_MS * 2, at));
    push_event(&mut events, &mut cache, Ev::You("hi".into(), Mark::Read, false));
    // a short pause: nothing
    assert!(!pause_mark(&mut events, &mut cache, PAUSE_MS - 1, at));
    assert!(pause_mark(&mut events, &mut cache, PAUSE_MS, at));
    // never two in a row
    assert!(!pause_mark(&mut events, &mut cache, PAUSE_MS, at));
    push_event(&mut events, &mut cache, Ev::Assistant("back".into()));
    let rows = cached_text(&events, &mut cache, 100);
    assert_eq!(rows, vec!["│  hi ✓✓", "", " · 14:31 ·", "", " back"], "{rows:#?}");
    // a mark ends a run of level 3: the run after it counts apart
    let mut evs = traffic(4, 2, 0);
    evs.push(Ev::TimeMark("15:02".into()));
    evs.extend(traffic(4, 2, 4));
    let (events, mut cache) = arrive(evs);
    let rows = cached_text(&events, &mut cache, 100);
    assert_eq!(rows[0], "▸ 4 messages between 2 agents");
    assert_eq!(rows[2], " · 15:02 ·");
    assert!(rows[4].starts_with("▸ 4 messages between 2 agents "));
    assert!(local_hhmm().len() == 5 && local_hhmm().as_bytes()[2] == b':');
}

#[test]
fn level_two_and_answered_lines() {
    let why = Ev::Answered {
        agent: "docs".into(),
        question: "v1 or v2?".into(),
        answer: "v2".into(),
        why: "the brief says v2.".into(),
        open: false,
    };
    let (mut events, mut cache) = arrive(vec![to_you("docs", "the examples use v2."), why]);
    let rows = cached_text(&events, &mut cache, 100);
    assert_eq!(rows, vec![" @ docs to you: the examples use v2.", "", " :* docs asked: v1 or v2? i answered: v2 ▸ why"]);
    assert!(toggle_event(&mut events, &mut cache, 1));
    let rows = cached_text(&events, &mut cache, 100);
    assert_eq!(rows[2..], [" :* docs asked: v1 or v2? i answered: v2 ▾ why", " │ the brief says v2."]);
    // a long level-3 message: 2 rows, `… ▸`; open, its whole text,
    // hung under its first column, `▾` at the end (BISE-106)
    let long = format!("heads-up: {}", "i'm touching web/src/auth ".repeat(8));
    let (mut events, mut cache) = arrive(vec![l3("auth-fix", "release", &long)]);
    let rows = cached_text(&events, &mut cache, 100);
    // BISE-127: the chip alone, the text under it at x0+2
    let hang = "  ";
    assert_eq!(rows.len(), 3, "{rows:#?}");
    assert_eq!(rows[0], chip_row("auth-fix", "release"));
    assert!(rows[1].starts_with("  heads-up") && rows[2].starts_with(hang), "{rows:#?}");
    assert!(!rows[2][hang.len()..].starts_with(' '), "the text starts at x0+2: {rows:#?}");
    assert!(rows[2].ends_with("… ▸"), "{rows:#?}");
    assert!(rows.iter().all(|r| r.width() <= CODE_MAX), "{rows:#?}");
    assert!(toggle_event(&mut events, &mut cache, 0));
    let rows = cached_text(&events, &mut cache, 100);
    assert!(rows.len() > 2 && rows[1..].iter().all(|r| r.starts_with(hang)), "{rows:#?}");
    assert!(rows.last().unwrap().ends_with("auth ▾") && !rows.iter().any(|r| r.contains('…')), "{rows:#?}");
}

#[test]
fn the_first_line_of_an_open_fold_toggles_by_row() {
    let long = format!("heads-up: {}", "x ".repeat(40));
    let mut evs = vec![Ev::You("go".into(), Mark::Read, false), l3("a", "b", &long)];
    evs.extend(traffic(4, 3, 0));
    let (mut events, mut cache) = arrive(evs);
    // row 1 (after the gap) is the fold line
    assert!(toggle_at(&mut events, &mut cache, 1, 1));
    assert!(matches!(events[1], Ev::AgentMsg { fold: true, open: false, .. }));
    // row 2 is its message: that opens the message
    assert!(toggle_at(&mut events, &mut cache, 1, 2));
    assert!(matches!(events[1], Ev::AgentMsg { fold: true, open: true, .. }));
    assert!(toggle_at(&mut events, &mut cache, 1, 1));
    assert!(matches!(events[1], Ev::AgentMsg { fold: false, .. }));
}

/// Mockup "what's for you, what isn't" (tui-screens.html).
#[test]
fn whats_for_you_matches_the_mockup() {
    let evs = vec![
        Ev::You("the login breaks on safari. and the api docs, v2 please.".into(), Mark::Read, false),
        Ev::Assistant("on it: auth-fix takes safari, docs takes the api docs.".into()),
        l3("docs", "main", "v1 or v2 for the examples?"),
        l3("main", "docs", "v2, the brief says so."),
        Ev::Answered { agent: "docs".into(), question: "v1 or v2 for the examples?".into(), answer: "v2".into(), why: "the brief says v2.".into(), open: false },
        l3("auth-fix", "release", "heads-up, i'm touching web/src/auth."),
        l3("release", "auth-fix", "ok, i'll mention the fix."),
        l3("bench", "main", "✓ done. p95 180 ms, 3 runs."),
        Ev::AgentMsg { from: "bench".into(), to: String::new(), text: "[report: done] p95 at 180 ms, nothing to fix.\nran it 3 times".into(), level: 3, id: String::new(), open: false, fold: false },
        l3("api-v2", "main", "? i need the v2 schema file, it isn't in the repo."),
        Ev::Card { text: "#4 question @api-v2 : the v2 schema file isn't in the repo. where is it?".into(), closed: String::new() },
    ];
    let (events, mut cache) = arrive(evs);
    let rows = cached_text(&events, &mut cache, 100);
    println!("{}", rows.join("\n"));
    let want: [&str; 23] = [
        "│  the login breaks on safari. and the api docs, v2 please. ✓✓",
        "",
        " on it: auth-fix takes safari, docs takes the api docs.",
        "",
        &chip_row("docs", "main"),
        &under("v1 or v2 for the examples?"),
        &chip_row("main", "docs"),
        &under("v2, the brief says so."),
        "",
        " :* docs asked: v1 or v2 for the examples? i answered: v2 ▸ why",
        "",
        &chip_row("auth-fix", "release"),
        &under("heads-up, i'm touching web/src/auth."),
        &chip_row("release", "auth-fix"),
        &under("ok, i'll mention the fix."),
        "",
        &chip_row("bench", "main"),
        &under("✓ done. p95 180 ms, 3 runs."),
        "",
        " ✓ bench: p95 at 180 ms, nothing to fix. ▸ report",
        "",
        &chip_row("api-v2", "main"),
        &under("? i need the v2 schema file, it isn't in the repo."),
    ];
    assert_eq!(rows[..want.len()], want[..], "{rows:#?}");
    assert!(rows[want.len()..].iter().any(|r| r.contains("api-v2 needs you")), "{rows:#?}");
}

/// Mockup "a busy hour, 30 agents" (tui-screens.html).
#[test]
fn a_busy_hour_matches_the_mockup() {
    let mut evs = vec![
        Ev::TimeMark("14:02".into()),
        Ev::You("ship the v2 api: endpoints, docs, sdk, migration, the lot.".into(), Mark::Read, false),
        Ev::Assistant("that's 30 pieces. i split it: 12 endpoints, 8 sdk, 6 docs, 4 migration. starting them.".into()),
    ];
    evs.extend(traffic(47, 30, 0));
    evs.push(Ev::Assistant("the 12 endpoint agents agreed on one error format; i picked it for the sdk agents too.".into()));
    evs.extend(traffic(23, 9, 100));
    evs.push(Ev::AgentMsg { from: "ep-users".into(), to: String::new(), text: "[report: done] 4 endpoints are done: users, orgs, keys, audit.\nmore".into(), level: 3, id: String::new(), open: false, fold: false });
    evs.extend([
        l3("sdk-py", "sdk-ts", "same pagination shape as you?"),
        l3("sdk-ts", "sdk-py", "yes: cursor + limit, max 200."),
        l3("mig-db", "main", "? drop the v1 tables now or after a release?"),
        l3("main", "mig-db", "asking the user."),
        l3("docs-auth", "ep-keys", "heads-up, i'm quoting your error codes."),
    ]);
    let opened = evs.len() - 5;
    evs.push(Ev::Card { text: "#7 question @mig-db : drop the v1 tables now, or keep them one release?".into(), closed: String::new() });
    evs.push(Ev::TimeMark("14:31".into()));
    evs.extend(traffic(12, 6, 200));
    let (mut events, mut cache) = arrive(evs);
    assert!(toggle_event(&mut events, &mut cache, opened));
    let rows = cached_text(&events, &mut cache, 100);
    println!("{}", rows.join("\n"));
    let pulse = crate::theme::working_frame(0).0;
    let want = [
        " · 14:02 ·".to_string(),
        "".into(),
        "│  ship the v2 api: endpoints, docs, sdk, migration, the lot. ✓✓".into(),
        "".into(),
        " that's 30 pieces. i split it: 12 endpoints, 8 sdk, 6 docs, 4 migration. starting them.".into(),
        "".into(),
        "▸ 47 messages between 30 agents".into(),
        "".into(),
        " the 12 endpoint agents agreed on one error format; i picked it for the sdk agents too.".into(),
        "".into(),
        "▸ 23 messages between 9 agents".into(),
        "".into(),
        " ✓ ep-users: 4 endpoints are done: users, orgs, keys, audit. ▸ report".into(),
        "".into(),
        "▾ 5 messages between 6 agents".into(),
        chip_row("sdk-py", "sdk-ts"),
        under("same pagination shape as you?"),
        chip_row("sdk-ts", "sdk-py"),
        under("yes: cursor + limit, max 200."),
        "".into(),
        chip_row("mig-db", "main"),
        under("? drop the v1 tables now or after a release?"),
        chip_row("main", "mig-db"),
        under("asking the user."),
        "".into(),
        chip_row("docs-auth", "ep-keys"),
        under("heads-up, i'm quoting your error codes."),
    ];
    assert_eq!(rows[..want.len()], want[..], "{rows:#?}");
    let tail: Vec<&String> = rows[want.len()..].iter().collect();
    assert!(tail.iter().any(|r| r.contains("mig-db needs you")), "{rows:#?}");
    assert_eq!(tail[tail.len() - 3..], [&" · 14:31 ·".to_string(), &String::new(), &format!("▸ 12 messages between 6 agents {}", pulse)]);
}

// ---- BISE-15: message marks (C3, book §13) ----

/// The wire lines into a feed, as run::ingest_line parses them.
fn ingest(lines: &[&str]) -> (Vec<Ev>, Vec<Option<EventRows>>) {
    let (mut events, mut cache) = (Vec::new(), Vec::new());
    for l in lines {
        let (line, replayed) = strip_history(l);
        let ev = if replayed { parse_history_line(line) } else { parse_line(line) };
        if let Some(ev) = ev {
            push_event(&mut events, &mut cache, ev);
        }
    }
    (events, cache)
}

fn marks(events: &[Ev]) -> Vec<(String, Mark)> {
    events
        .iter()
        .filter_map(|e| match e {
            Ev::You(t, m, ..) => Some((t.clone(), *m)),
            _ => None,
        })
        .collect()
}

#[test]
fn steering_moves_the_mark_of_your_line() {
    // your line while the agent works: `·`, then `✓` (received), `✓✓` (read)
    let (mut events, mut cache) = ingest(&[
        "sb you : run the tests",
        "  obs: turn_started",
        "sb you : skip firefox",
    ]);
    let rows = cached_text(&events, &mut cache, 100);
    assert!(rows.contains(&format!("│  skip firefox {}", G_SENDING)), "{rows:#?}");
    push_event(&mut events, &mut cache, parse_line("  obs: steering_received: skip firefox").unwrap());
    let rows = cached_text(&events, &mut cache, 100);
    assert!(rows.contains(&format!("│  skip firefox {}", G_RECEIVED)), "{rows:#?}");
    push_event(&mut events, &mut cache, parse_line("  obs: steered: skip firefox").unwrap());
    let rows = cached_text(&events, &mut cache, 100);
    assert!(rows.contains(&format!("│  skip firefox {}", G_READ)), "{rows:#?}");
    // no info line for either: the mark says it
    assert!(!rows.iter().any(|r| r.contains("steer")), "{rows:#?}");
    // the colors: `·` dim, `✓` faint, `✓✓` accent
    let last = |evs: &[Ev]| -> Style {
        let rows = build_rows(evs, evs.len() - 1, false, 100, 0);
        rows.iter().flat_map(|r| r.spans.clone()).last().unwrap().style
    };
    let one = |m| vec![Ev::You("x".into(), m, false)];
    assert_eq!(last(&one(Mark::Sent)).fg, Some(crate::theme::dim()));
    assert_eq!(last(&one(Mark::Received)).fg, Some(crate::theme::faint()));
    assert_eq!(last(&one(Mark::Read)).fg, Some(crate::theme::accent()));
}

#[test]
fn a_mark_only_moves_up_and_finds_its_line() {
    let (mut events, mut cache) = ingest(&[
        "sb you : same text",
        "  obs: turn_started",
        "sb you : other",
        "sb you : same   text",
    ]);
    // a line break flattened on the wire still matches; the latest line
    // with the text gets it
    push_event(&mut events, &mut cache, parse_line("  obs: steered: same text").unwrap());
    push_event(&mut events, &mut cache, parse_line("  obs: steering_received: same text").unwrap());
    assert_eq!(
        marks(&events),
        vec![("same text".into(), Mark::Read), ("other".into(), Mark::Sent), ("same   text".into(), Mark::Read)]
    );
    // a steering line with no line of yours: nothing appended
    let n = events.len();
    push_event(&mut events, &mut cache, parse_line("  obs: steered: unknown").unwrap());
    assert_eq!(events.len(), n);
}

#[test]
fn a_message_at_idle_is_read_when_its_turn_starts() {
    let (events, _) = ingest(&["sb you : hello", "  obs: turn_started", "  obs: assistant: hi", "--- idle", "sb you : next"]);
    assert_eq!(marks(&events), vec![("hello".into(), Mark::Read), ("next".into(), Mark::Sent)]);
}

#[test]
fn a_replayed_history_restores_the_marks() {
    // --resume: `you :` lines were committed (read); a steering the Core
    // committed comes back as `injected :` and marks its line; an
    // injected notification stays an info line
    let (events, mut cache) = ingest(&[
        "history you : fix the login",
        "history   obs: turn_started",
        "history injected : skip firefox",
        "history injected : [notification] docs is done",
    ]);
    assert_eq!(marks(&events), vec![("fix the login".into(), Mark::Read)]);
    let rows = cached_text(&events, &mut cache, 100);
    // a replayed steering has no `you :` line of its own: it stays the
    // injected line (the REPL history can't tell it from a notification)
    assert!(rows.iter().any(|r| r.contains("injected · skip firefox")), "{rows:#?}");
    assert!(rows.iter().any(|r| r.contains("injected · [notification] docs is done")), "{rows:#?}");
    // with its line in the feed (typed live, then the REPL replays), the
    // injected line marks it read and adds nothing
    let (events, mut cache) = ingest(&["sb you : skip firefox", "history injected : skip firefox"]);
    assert_eq!(marks(&events), vec![("skip firefox".into(), Mark::Read)]);
    assert_eq!(cached_text(&events, &mut cache, 100), vec![format!("│  skip firefox {}", G_READ)]);
    // a steering typed live then replayed from the hub transcript: the
    // same obs lines give the same marks
    let (events, _) = ingest(&[
        "sb you : fix the login",
        "  obs: turn_started",
        "sb you : skip firefox",
        "  obs: steering_received: skip firefox",
        "sb you : and log the headers",
        "  obs: steering_received: and log the headers",
        "  obs: steered: skip firefox",
        "sb you : also logout",
    ]);
    assert_eq!(
        marks(&events),
        vec![
            ("fix the login".into(), Mark::Read),
            ("skip firefox".into(), Mark::Read),
            ("and log the headers".into(), Mark::Received),
            ("also logout".into(), Mark::Sent),
        ]
    );
}

#[test]
fn mains_replies_carry_its_glyph_in_its_feed_only() {
    let (events, mut cache) = arrive(vec![Ev::Assistant("on it: auth-fix takes safari.".into())]);
    crate::render::set_main_feed(true);
    let in_main = cached_text(&events, &mut cache, 100);
    // the rows built for main's feed rebuild inside an agent
    crate::render::set_main_feed(false);
    let in_agent = cached_text(&events, &mut cache, 100);
    assert_eq!(in_main, vec![format!(" {} on it: auth-fix takes safari.", crate::theme::G_MAIN)]);
    assert_eq!(in_agent, vec![" on it: auth-fix takes safari.".to_string()]);
}

// BISE-90: the hub steered your message together with an agent's message
// and the agents' status: the steered text is that block, not your
// words; it still moves the marks of what you sent during this turn
#[test]
fn a_combined_steer_moves_the_marks_of_this_turn() {
    let mut events: Vec<Ev> = Vec::new();
    let mut cache: Vec<Option<EventRows>> = Vec::new();
    push_event(&mut events, &mut cache, Ev::You("before".into(), Mark::Sent, false));
    push_event(&mut events, &mut cache, Ev::Turn);
    push_event(&mut events, &mut cache, Ev::You("also check the logs".into(), Mark::Sent, false));
    let block = "<agent_message from=\"noisy\" relation=\"child\" id=\"m_4\">";
    let mark = |m| Ev::MarkYou { text: block.into(), mark: m, or: None };
    assert!(!push_event(&mut events, &mut cache, mark(Mark::Received)));
    assert!(matches!(&events[2], Ev::You(_, Mark::Received, ..)));
    assert!(!push_event(&mut events, &mut cache, mark(Mark::Read)));
    assert!(matches!(&events[2], Ev::You(_, Mark::Read, ..)));
    // a message of an earlier turn keeps its own mark (Turn read it)
    assert!(matches!(&events[0], Ev::You(_, Mark::Read, ..)));
    assert_eq!(events.len(), 3);
}

// BISE-90: an opened report whose first line is longer than the row hangs
// its continuation under the text, never at column 1
#[test]
fn an_open_report_hangs_its_rows() {
    let long = "p95 at 180 ms, nothing to fix, the slow tail is the cold cache on the first request of each worker";
    let ev = Ev::AgentMsg {
        from: "bench".into(),
        to: String::new(),
        text: format!("[report: done] {}", long),
        level: 3,
        id: String::new(),
        open: true,
        fold: false,
    };
    let rows = rows_text(&ev_rows(&ev, 0, 60));
    assert!(rows.len() >= 2, "{rows:#?}");
    assert!(rows[0].starts_with(" ✓ bench: p95"), "{rows:#?}");
    for r in &rows[1..] {
        assert!(r.starts_with("   ") && !r.starts_with("    "), "{r:?} in {rows:#?}");
    }
}

// BISE-97: main's reply is wrapped once, at its text's width; the rows
// under the first line up with its text (at 80 columns a row 1 cell too
// wide used to wrap again, leaving a one-word row)
#[test]
fn mains_reply_wraps_once_under_its_text() {
    let long = "the quick brown fox jumps over the lazy dog and keeps running across the field until the evening comes and the light fades away slowly";
    crate::render::set_main_feed(true);
    for w in 40..110usize {
        let rows: Vec<String> = crate::render::ev_rows(&Ev::Assistant(long.into()), 0, w)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        let text_w = crate::render::prose_width(w) - 4;
        // no row is cut short: each one but the last could not take the next word
        for pair in rows.windows(2) {
            let next = pair[1].split_whitespace().next().unwrap();
            assert!(pair[0].trim_start_matches([' ', ':', '*']).chars().count() + 1 + next.chars().count() > text_w, "w {}: {:?}", w, rows);
        }
        for r in &rows[1..] {
            assert!(r.starts_with("    ") && !r[4..].starts_with(' '), "w {}: {:?}", w, rows);
        }
    }
    crate::render::set_main_feed(false);
}

#[test]
fn session_log_facts_are_not_in_the_feed() {
    // BISE-195: the REPL's `ev:` lines go to the hub's session writer
    assert!(parse_line(r#"  ev: {"type":"turn_started","v":1,"data":{"cause":"user"}}"#).is_none());
}

// ---- BISE-223: a call in main is one row (the model's description) ----

/// The wire lines of one call: started, its code and intent, its result
/// and its end (`ok`: None while it runs).
fn call_lines(id: u32, name: &str, code: &str, intent: Option<&str>, done: Option<(bool, &str)>) -> Vec<String> {
    let mut l = vec![format!("  obs: tool_started #{id}"), format!("tool #{id} {name} : x"), format!("tool_code #{id} : {}", wire_encode(code))];
    if let Some(d) = intent {
        l.push(format!("tool_intent #{id} : {d}"));
    }
    if let Some((ok, out)) = done {
        l.push(format!("tool_result #{id} {} : {out}", if ok { "ok" } else { "fail" }));
        l.push(format!("  obs: tool_finished #{id} {}", if ok { "ok" } else { "failed" }));
    }
    l
}

fn feed_of(lines: &[String]) -> (Vec<Ev>, Vec<Option<EventRows>>) {
    let mut events: Vec<Ev> = Vec::new();
    let mut cache: Vec<Option<EventRows>> = Vec::new();
    for l in lines {
        if let Some(ev) = parse_line(l) {
            push_event(&mut events, &mut cache, ev);
        }
    }
    (events, cache)
}

/// The rows of a feed, trimmed, the elapsed times masked (`0.0s` → `T`).
fn main_text(events: &[Ev], width: usize) -> Vec<String> {
    let re = |s: String| {
        let mut out = String::new();
        let mut rest = s.as_str();
        while let Some(p) = rest.find(|c: char| c.is_ascii_digit()) {
            let tail = &rest[p..];
            let n = tail.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(tail.len());
            if tail[n..].starts_with('s') && !rest[..p].ends_with("exit ") {
                out.push_str(&rest[..p]);
                out.push('T');
                rest = &tail[n + 1..];
            } else {
                out.push_str(&rest[..p + n]);
                rest = &tail[n..];
            }
        }
        out.push_str(rest);
        out
    };
    feed_text(events, width).into_iter().map(re).collect()
}

#[test]
fn a_call_in_main_is_one_row_with_its_description() {
    crate::render::set_main_feed(true);
    let mut lines = call_lines(1, "bash", "git log --oneline -5", Some("checking what changed in signup"), Some((true, "a91c2e0 hero")));
    lines.extend(call_lines(2, "run_typescript", "{\"code\":\"return 1\"}", Some("looking for timeouts in Sentry"), Some((true, "1"))));
    lines.extend(call_lines(3, "bash", "python3 repro.py", Some("reproducing the crash"), Some((false, "exit 1: Traceback (most recent call last):   File x UnicodeDecodeError: 'latin-1' codec can't decode"))));
    lines.extend(call_lines(4, "bash", "npm test", Some("running the export tests"), None));
    let (events, _) = feed_of(&lines);
    let rows = main_text(&events, 60);
    crate::render::set_main_feed(false);
    assert_eq!(
        rows,
        vec![
            " $ checking what changed in signup                    ✓ T",
            " ƒ looking for timeouts in Sentry                     ✓ T",
            " $ reproducing the crash                     ✗ exit 1 · T",
            "   UnicodeDecodeError: 'latin-1' codec can't decode",
            " $ running the export tests                           ∿ T",
        ]
    );
}

#[test]
fn a_row_without_description_shows_the_first_line_and_cuts_long_text() {
    crate::render::set_main_feed(true);
    let mut lines = call_lines(1, "bash", "\n  grep -rn latin api/\nls", None, Some((true, "x")));
    let long = "checking whether any customer exports still expect latin-1 before we switch";
    lines.extend(call_lines(2, "bash", "true", Some(long), Some((true, ""))));
    let (events, _) = feed_of(&lines);
    let rows = main_text(&events, 50);
    crate::render::set_main_feed(false);
    assert_eq!(rows[0], " $ grep -rn latin api/                      ✓ T");
    assert!(rows[1].starts_with(" $ checking whether any customer exports s…"), "{rows:#?}");
    assert!(rows[1].ends_with("✓ T"), "{rows:#?}");
}

#[test]
fn four_done_calls_fold_and_the_failed_and_running_keep_their_rows() {
    crate::render::set_main_feed(true);
    let mut lines = Vec::new();
    for id in 1..=3 {
        lines.extend(call_lines(id, "bash", "ls", Some(&format!("step {id}")), Some((true, "ok"))));
    }
    lines.extend(call_lines(4, "bash", "make docs", Some("building the docs"), Some((false, "exit 2: broken link: docs/dark.png"))));
    lines.extend(call_lines(5, "bash", "ls", Some("step 5"), Some((true, "ok"))));
    lines.extend(call_lines(6, "run_typescript", "{\"code\":\"x\"}", Some("asking GitHub"), None));
    let (mut events, mut cache) = feed_of(&lines);
    let rows = main_text(&events, 60);
    assert_eq!(
        rows,
        vec![
            " $ ▸ 4 commands · step 1                              ✓ T",
            " $ building the docs                         ✗ exit 2 · T",
            "   broken link: docs/dark.png",
            " ƒ asking GitHub                                      ∿ T",
        ]
    );
    // a click on the fold: the rows come back, in place
    assert!(toggle_event(&mut events, &mut cache, 0));
    let open = main_text(&events, 60);
    assert_eq!(open[0], " $ ▾ 4 commands · step 1                              ✓ T");
    assert_eq!(open[1], " $ step 1                                             ✓ T");
    assert_eq!(open.len(), 8, "{open:#?}");
    crate::render::set_main_feed(false);
}

#[test]
fn a_click_opens_one_box_and_ctrl_o_opens_every_box_then_back_to_rows() {
    crate::render::set_main_feed(true);
    let mut lines = call_lines(1, "bash", "du -h hero.png", Some("weighing the hero image"), Some((true, "4.2M  hero.png")));
    lines.extend(call_lines(2, "bash", "ls", Some("listing"), Some((true, "a"))));
    let (mut events, mut cache) = feed_of(&lines);
    assert!(toggle_event(&mut events, &mut cache, 0));
    let one = main_text(&events, 60);
    assert!(one[0].starts_with("╭─ $ weighing the hero image ✓ T ─"), "{one:#?}");
    assert!(one.iter().any(|r| r.starts_with("│ du -h hero.png")), "{one:#?}");
    assert_eq!(one.last().unwrap(), " $ listing                                            ✓ T");
    // ctrl+o: every box; again: every row
    assert!(anything_closed(&events));
    set_everything(&mut events, &mut cache, true);
    let all = main_text(&events, 60);
    assert_eq!(all.iter().filter(|r| r.starts_with("╭─ $ ")).count(), 2, "{all:#?}");
    assert!(!anything_closed(&events));
    set_everything(&mut events, &mut cache, false);
    let rows = main_text(&events, 60);
    assert_eq!(rows.len(), 2, "{rows:#?}");
    crate::render::set_main_feed(false);
}

#[test]
fn an_agent_view_shows_rows_and_ctrl_o_opens_the_boxes() {
    // BISE-223: an agent's view draws its calls as rows, as main does
    crate::render::set_main_feed(false);
    let mut lines = call_lines(1, "run_typescript", "{\"code\":\"return 1\"}", Some("looking for timeouts"), Some((true, "1")));
    lines.extend(call_lines(2, "bash", "ls", None, Some((true, "a"))));
    let (mut events, mut cache) = feed_of(&lines);
    let rows = main_text(&events, 60);
    assert_eq!(
        rows,
        vec![
            " ƒ looking for timeouts                               ✓ T",
            " $ ls                                                 ✓ T",
        ]
    );
    // ctrl+o: every box, the description as title (no description: the
    // kind's word); again: every row
    assert!(anything_closed(&events));
    set_everything(&mut events, &mut cache, true);
    let all = main_text(&events, 60);
    assert!(all[0].starts_with("╭─ ƒ looking for timeouts ✓ T ─"), "{all:#?}");
    assert!(all.iter().any(|r| r.starts_with("╭─ $ bash ✓ T ─")), "{all:#?}");
    assert!(!anything_closed(&events));
    set_everything(&mut events, &mut cache, false);
    assert_eq!(main_text(&events, 60), rows);
    // a click (space) opens one
    assert!(toggle_event(&mut events, &mut cache, 0));
    let one = main_text(&events, 60);
    assert!(one[0].starts_with("╭─ ƒ looking for timeouts ✓ T ─"), "{one:#?}");
    assert_eq!(one.last().unwrap(), &rows[1]);
}

#[test]
fn an_agent_view_folds_four_done_calls() {
    crate::render::set_main_feed(false);
    let mut lines = Vec::new();
    for n in 1..=4 {
        lines.extend(call_lines(n, "bash", "true", Some(&format!("step {n}")), Some((true, "ok"))));
    }
    let (events, _) = feed_of(&lines);
    let rows = main_text(&events, 60);
    assert_eq!(rows, vec![" $ ▸ 4 commands · step 1                              ✓ T"], "{rows:#?}");
}

#[test]
fn the_ascii_rows() {
    crate::theme::set_ascii_for_tests(true);
    crate::render::set_main_feed(true);
    let mut lines = call_lines(1, "run_typescript", "{\"code\":\"x\"}", Some("looking"), Some((true, "1")));
    lines.extend(call_lines(2, "bash", "false", Some("failing"), Some((false, "exit 1: Error: boom"))));
    lines.extend(call_lines(3, "bash", "sleep 9", Some("waiting"), None));
    let (events, _) = feed_of(&lines);
    let rows = main_text(&events, 40);
    crate::render::set_main_feed(false);
    crate::theme::set_ascii_for_tests(false);
    assert_eq!(
        rows,
        vec![
            " f looking                       ok T",
            " $ failing               x exit 1 · T",
            "   Error: boom",
            " $ waiting                        ~ T",
        ]
    );
}

// ---- BISE-304: a run of edits folds into one row ----

/// An edit call's wire lines: its patch (`+` adds, `-` removes).
fn edit_lines(id: u32, name: &str, path: &str, adds: usize, dels: usize, done: Option<(bool, &str)>) -> Vec<String> {
    let mut p = format!("*** Begin Patch
*** Update File: {path}
@@
");
    p.push_str(&"-old
".repeat(dels));
    p.push_str(&"+new
".repeat(adds));
    p.push_str("*** End Patch
");
    call_lines(id, name, &p, None, done)
}

#[test]
fn three_done_edits_fold_and_the_failed_one_keeps_its_row() {
    let mut lines = edit_lines(1, "write_file", "src/auth/session.ts", 3, 0, Some((true, "ok")));
    lines.extend(edit_lines(2, "edit", "src/auth/token.ts", 1, 1, Some((true, "ok"))));
    lines.extend(edit_lines(3, "edit", "src/auth/nope.ts", 1, 1, Some((false, "File does not exist"))));
    lines.extend(edit_lines(4, "edit", "src/auth/session.ts", 1, 1, Some((true, "ok"))));
    lines.extend(edit_lines(5, "apply_patch", "README.md", 2, 0, Some((true, "ok"))));
    let (mut events, mut cache) = feed_of(&lines);
    let rows = main_text(&events, 70);
    assert_eq!(
        rows,
        vec![
            " ± ▸ 3 files · session.ts, token.ts, README.md                 ✓ +7 −2",
            " ± edit src/auth/nope.ts ✗ File does not exist ▸",
        ],
        "{rows:#?}"
    );
    // a click: the rows come back in place, each still opens its diff
    assert!(toggle_event(&mut events, &mut cache, 0));
    let open = main_text(&events, 70);
    assert_eq!(open[0], " ± ▾ 3 files · session.ts, token.ts, README.md                 ✓ +7 −2");
    assert_eq!(open[1], " ± write src/auth/session.ts ✓ +3 ▸");
    assert_eq!(open.len(), 6, "{open:#?}");
    // ctrl+o: the fold and every diff; again: all closed
    set_everything(&mut events, &mut cache, false);
    assert_eq!(main_text(&events, 70), rows);
    set_everything(&mut events, &mut cache, true);
    let all = main_text(&events, 70);
    assert!(all[0].starts_with(" ± ▾ 3 files"), "{all:#?}");
    assert!(all.iter().any(|r| r.contains("+new")), "{all:#?}");
}

#[test]
fn two_edits_stay_rows_and_a_command_ends_the_run() {
    let mut lines = edit_lines(1, "edit", "a.ts", 1, 0, Some((true, "ok")));
    lines.extend(edit_lines(2, "edit", "b.ts", 1, 0, Some((true, "ok"))));
    lines.extend(call_lines(3, "bash", "ls", Some("checking"), Some((true, "ok"))));
    lines.extend(edit_lines(4, "edit", "c.ts", 1, 0, Some((true, "ok"))));
    lines.extend(edit_lines(5, "edit", "d.ts", 1, 0, None));
    let (events, _) = feed_of(&lines);
    let rows = main_text(&events, 60);
    assert_eq!(rows.len(), 5, "{rows:#?}");
    assert!(rows.iter().all(|r| !r.contains("files ·")), "{rows:#?}");
}

#[test]
fn one_file_counts_the_edits_and_names_fit_whole() {
    let mut lines = Vec::new();
    for id in 1..=4 {
        lines.extend(edit_lines(id, "edit", "web/src/auth/session.ts", 2, 1, Some((true, "ok"))));
    }
    let (events, _) = feed_of(&lines);
    assert_eq!(main_text(&events, 60), vec![" ± ▸ 4 edits · session.ts                            ✓ +8 −4"]);
    // many files: whole names, then `+n more`; a basename twice gets its parent
    let names = ["auth/index.ts", "ui/index.ts", "router.ts", "store.ts", "theme.ts"];
    let mut lines = Vec::new();
    for (id, p) in names.iter().enumerate() {
        lines.extend(edit_lines(id as u32 + 1, "edit", p, 1, 0, Some((true, "ok"))));
    }
    let (events, _) = feed_of(&lines);
    assert_eq!(main_text(&events, 60), vec![" ± ▸ 5 files · auth/index.ts, ui/index.ts +3 more       ✓ +5"]);
    // not even one name fits: it is cut
    assert_eq!(main_text(&events, 30), vec![" ± ▸ 5 files · a… +4 more ✓ +5"]);
}

#[test]
fn the_ascii_edits_fold() {
    crate::theme::set_ascii_for_tests(true);
    let mut lines = Vec::new();
    for (id, p) in ["a.ts", "b.ts", "c.ts"].iter().enumerate() {
        lines.extend(edit_lines(id as u32 + 1, "edit", p, 1, 1, Some((true, "ok"))));
    }
    let (events, _) = feed_of(&lines);
    let rows = main_text(&events, 50);
    crate::theme::set_ascii_for_tests(false);
    assert_eq!(rows, vec![" % > 3 files · a.ts, b.ts, c.ts           ok +3 -3"]);
}

// ---- a long message of yours folds (BISE-239) ----

#[test]
fn a_long_message_of_yours_folds_to_twenty_rows_and_its_hint() {
    let text = (1..=30).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n");
    let (mut events, mut cache) = arrive(vec![Ev::You(text, Mark::Read, false)]);
    let rows = cached_text(&events, &mut cache, 60);
    let body: Vec<&String> = rows.iter().filter(|r| !r.trim().is_empty()).collect();
    // 20 rows of text (BISE-262; 12 before, 8 at first), then the hint
    // on its own row, the bar through it
    assert_eq!(YOU_ROWS, 20);
    assert_eq!(body.len(), YOU_ROWS + 1, "{rows:#?}");
    assert_eq!(body[0].as_str(), "│  line 1", "{rows:#?}");
    assert_eq!(body[19].as_str(), "│  line 20", "{rows:#?}");
    assert_eq!(body[20].as_str(), "│  ▸ 10 more lines ✓✓", "{rows:#?}");
    assert!(crate::feed::is_closed_at(&events, 0) && crate::feed::anything_closed(&events));
    // ctrl+o opens it whole, `▾` after its last line; again folds it
    crate::feed::set_everything(&mut events, &mut cache, true);
    let open = cached_text(&events, &mut cache, 60);
    assert!(open.iter().any(|r| r == "│  line 30 ▾ ✓✓") && !open.iter().any(|r| r.contains("more line")), "{open:#?}");
    assert_eq!(open.iter().filter(|r| r.starts_with("│  line")).count(), 30);
    crate::feed::set_everything(&mut events, &mut cache, false);
    assert!(cached_text(&events, &mut cache, 60).iter().any(|r| r.contains("▸ 10 more lines")));
}

#[test]
fn a_line_cut_to_fit_counts_as_hidden() {
    // 19 short lines then one that wraps over rows 20-22: rows 1-20
    // show, the 20th line is cut, so 1 more line (and the 2 after it: 3)
    let mut lines: Vec<String> = (1..=19).map(|n| format!("l{n}")).collect();
    lines.push("word ".repeat(30));
    lines.push("a".into());
    lines.push("b".into());
    let (events, mut cache) = arrive(vec![Ev::You(lines.join("\n"), Mark::Read, false)]);
    let rows = cached_text(&events, &mut cache, 60);
    let body: Vec<&String> = rows.iter().filter(|r| !r.trim().is_empty()).collect();
    assert_eq!(body.len(), YOU_ROWS + 1, "{rows:#?}");
    assert!(body[19].starts_with("│  word"), "{rows:#?}");
    assert_eq!(body[20].as_str(), "│  ▸ 3 more lines ✓✓", "{rows:#?}");
}

#[test]
fn a_short_message_of_yours_stays_whole() {
    let text = (1..=20).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n");
    let (events, mut cache) = arrive(vec![Ev::You(text, Mark::Read, false)]);
    let rows = cached_text(&events, &mut cache, 60);
    assert_eq!(rows.iter().filter(|r| r.starts_with("│  line")).count(), 20, "{rows:#?}");
    assert!(rows.iter().any(|r| r == "│  line 20 ✓✓") && !rows.iter().any(|r| r.contains("more line") || r.contains('▾')), "{rows:#?}");
    assert!(!crate::feed::discloses(&events[0]) && !crate::feed::anything_closed(&events));
}

#[test]
fn a_click_on_the_hint_row_opens_just_that_message() {
    let long = (1..=24).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n");
    let (mut events, mut cache) = arrive(vec![Ev::You(long.clone(), Mark::Read, false), Ev::You(long, Mark::Read, false)]);
    let rows = cached_text(&events, &mut cache, 60);
    let n = cache[1].as_ref().unwrap().rows.len();
    assert!(rows.iter().filter(|r| r.contains("▸ 4 more lines")).count() == 2, "{rows:#?}");
    // a click on a text row does nothing; on the hint row it opens
    assert!(!toggle_at(&mut events, &mut cache, 1, n - 3));
    assert!(matches!(events[1], Ev::You(_, _, false)));
    assert!(toggle_at(&mut events, &mut cache, 1, n - 1));
    assert!(matches!(events[1], Ev::You(_, _, true)) && matches!(events[0], Ev::You(_, _, false)));
    // open: its last row (the `▾` one) folds it again
    let _ = cached_text(&events, &mut cache, 60);
    let n = cache[1].as_ref().unwrap().rows.len();
    assert!(toggle_at(&mut events, &mut cache, 1, n - 1));
    assert!(matches!(events[1], Ev::You(_, _, false)));
}

/// A long message of yours with a few screenshots: the sizes of its
/// images wrap to several rows under the hint row; a click on the hint
/// still opens it (the click took the sizes for one row and missed it),
/// the sizes rows stay inert, the `▾` row folds it again.
#[test]
fn a_click_on_the_hint_opens_a_message_whose_image_sizes_wrap() {
    let shot = |n: u32| format!("<image name=\"[Image #{n}]\" path=\"/shots/Screenshot 2026-10-02 at 11.3{n}.59.png\" mime=\"image/png\" b64=\"/nowhere/{n}.b64\">");
    let long = (1..=24).map(|n| format!("{n}. {} line {n}", shot(n % 6))).collect::<Vec<_>>().join("\n");
    let (mut events, mut cache) = arrive(vec![Ev::You(long, Mark::Read, false)]);
    let rows = cached_text(&events, &mut cache, 60);
    let n = cache[0].as_ref().unwrap().rows.len();
    let hint = rows.iter().position(|r| r.contains("▸") && r.contains("more lines")).expect("a hint row");
    let sizes = rows.len() - 1 - hint;
    assert!(sizes >= 3, "the sizes wrap: {rows:#?}");
    assert!(rows[hint + 1..].iter().all(|r| r.starts_with("│  ") && r.contains("Screenshot")), "{rows:#?}");
    let hint = hint - (rows.len() - n);
    // the sizes rows and the text rows: nothing; the hint row: opens
    for r in (hint + 1..n).chain([hint - 1]) {
        assert!(!crate::feed::toggles_at(&events, &cache, 0, r) && !toggle_at(&mut events, &mut cache, 0, r), "row {r}");
    }
    assert!(matches!(events[0], Ev::You(_, _, false)));
    assert!(crate::feed::toggles_at(&events, &cache, 0, hint));
    assert!(toggle_at(&mut events, &mut cache, 0, hint));
    assert!(matches!(events[0], Ev::You(_, _, true)));
    // open: the `▾` row (its last line, above the sizes) folds it again
    let rows = cached_text(&events, &mut cache, 60);
    let n = cache[0].as_ref().unwrap().rows.len();
    let open = rows.iter().position(|r| r.contains("line 24") && r.contains('▾')).expect("the ▾ row") - (rows.len() - n);
    assert!(!toggle_at(&mut events, &mut cache, 0, n - 1));
    assert!(toggle_at(&mut events, &mut cache, 0, open));
    assert!(matches!(events[0], Ev::You(_, _, false)));
}

/// BISE-240: a long paste in your message is its chip in the line and
/// one dim row under it; a click on that row (or ctrl+o) opens the
/// message and shows the full text there, never by default.
#[test]
fn a_long_paste_in_your_message_is_a_chip_and_opens_like_a_fold() {
    let body: String = (1..=40).map(|n| format!("row {n}\n")).collect();
    let msg = format!("see {} ok", crate::pasted::tag(1, body.trim_end()));
    let (mut events, mut cache) = arrive(vec![Ev::You(msg, Mark::Read, false)]);
    assert!(crate::feed::discloses(&events[0]) && crate::feed::anything_closed(&events));
    let rows = cached_text(&events, &mut cache, 80);
    let mine: Vec<&String> = rows.iter().filter(|r| r.starts_with('│')).collect();
    assert_eq!(mine.len(), 2, "{rows:#?}");
    assert_eq!(mine[0].as_str(), "│  see ▤ 1 ok ✓✓", "{rows:#?}");
    assert!(mine[1].starts_with("│  ▤ 1 “row 1 row 2") && mine[1].ends_with("” · 40 lines"), "{rows:#?}");
    assert!(!rows.iter().any(|r| r.contains("row 40")), "{rows:#?}");
    // a click on the paste row opens it: the full text under the line
    let n = cache[0].as_ref().unwrap().rows.len();
    assert!(toggle_at(&mut events, &mut cache, 0, n - 1));
    assert!(matches!(events[0], Ev::You(_, _, true)));
    let rows = cached_text(&events, &mut cache, 80);
    assert!(rows.iter().any(|r| r == "│  ▤ 1 · 40 lines"), "{rows:#?}");
    assert!(rows.iter().any(|r| r == "│  row 40"), "{rows:#?}");
    // its last row closes it again
    let n = cache[0].as_ref().unwrap().rows.len();
    assert!(toggle_at(&mut events, &mut cache, 0, n - 1));
    assert!(matches!(events[0], Ev::You(_, _, false)));
}

/// A skill call as the wire brings it: `{"name":"<skill>"}`, then its
/// result (`None`: still running).
fn skill_call(id: u32, skill: &str, result: Option<(bool, &str)>) -> Ev {
    let mut lines = vec![format!("  obs: tool_started #{id}"), format!("tool #{id} skill : {{\"name\":\"{skill}\"}}")];
    if let Some((ok, r)) = result {
        lines.push(format!("tool_result #{id} {} : {r}", if ok { "ok" } else { "fail" }));
        lines.push(format!("  obs: tool_finished #{id} {}", if ok { "ok" } else { "failed" }));
    }
    Ev::Tool(merged_tool(&lines))
}

/// BISE-283 (the user's onboarding test, designer's rows): a skill call
/// is a sentence, `reading skill <name>` then `read skill <name>`, never
/// `skill ✓ 0.0s · {"name":…}` and `▸ output`; a failed one says why on
/// the right; a click (or ctrl+o) opens its SKILL.md in a box.
#[test]
fn a_skill_call_reads_as_a_sentence() {
    let body = "# bise demo

Show what bise can do.";
    let run = skill_call(1, "bise-demo", None);
    let rows = rows_text(&build_rows(std::slice::from_ref(&run), 0, false, 60, 0));
    assert!(rows[0].starts_with("   reading skill bise-demo "), "{rows:#?}");
    assert_eq!(rows.len(), 1, "{rows:#?}");
    // the verb dim, the name in the text color while it runs
    let spans = &build_rows(std::slice::from_ref(&run), 0, false, 60, 0)[0].spans;
    let name = spans.iter().find(|s| s.content == "bise-demo").unwrap();
    assert_eq!(name.style.fg, Some(crate::theme::text()));
    let verb = spans.iter().find(|s| s.content.starts_with("reading")).unwrap();
    assert_eq!(verb.style.fg, Some(crate::theme::dim()));

    let done = skill_call(2, "bise-demo", Some((true, body)));
    let (mut events, mut cache) = arrive(vec![done]);
    let rows = cached_text(&events, &mut cache, 60);
    assert_eq!(rows, vec!["   read skill bise-demo".to_string()], "{rows:#?}");
    assert!(crate::feed::anything_closed(&events));
    // ctrl+o: the SKILL.md in a box titled with the sentence, no JSON
    crate::feed::set_everything(&mut events, &mut cache, true);
    let rows = cached_text(&events, &mut cache, 60);
    assert!(rows[0].starts_with("╭─ read skill bise-demo ✓"), "{rows:#?}");
    assert!(rows.iter().any(|r| r.starts_with("│ Show what bise can do.")), "{rows:#?}");
    assert!(!rows.iter().any(|r| r.contains('{') || r.starts_with('├')), "{rows:#?}");
    crate::feed::set_everything(&mut events, &mut cache, false);
    assert_eq!(cached_text(&events, &mut cache, 60), vec!["   read skill bise-demo".to_string()]);

    let why = "unknown skill: bise-dmeo: call it with an exact name from <available-skills>";
    let failed = skill_call(3, "bise-dmeo", Some((false, why)));
    let (events, mut cache) = arrive(vec![failed]);
    let rows = cached_text(&events, &mut cache, 60);
    assert_eq!(rows.len(), 1, "{rows:#?}");
    assert!(rows[0].starts_with("   read skill bise-dmeo ") && rows[0].ends_with("✗ unknown skill"), "{rows:#?}");
    // the other views draw the same row
    let other = text_of(&events[0], 60);
    assert_eq!(other.len(), 1, "{other:#?}");
    assert!(other[0].trim_end().ends_with("✗ unknown skill"), "{other:#?}");
}

// BISE-293: a refused request says it once, in two lines: bise's, then
// the provider's own words dim under it (never the raw JSON twice)
#[test]
fn refused_request_reads_once_in_two_lines() {
    let why = "OpenAI refused the request (400). OpenAI said: \"Invalid 'tools[0].function.name': string does not match pattern.\"";
    let mut events: Vec<Ev> = Vec::new();
    let mut cache: Vec<Option<EventRows>> = Vec::new();
    for l in [format!("  obs: candidate_discarded: {}", why), format!("  obs: turn_done: failed: {}", why)] {
        push_event(&mut events, &mut cache, parse_line(&l).expect("parse"));
    }
    assert_eq!(events.len(), 1, "the warning gives its place to the failure");
    let rows = rows_text(&ev_lines(&events[0], 200));
    assert_eq!(rows.len(), 2, "{:?}", rows);
    assert_eq!(rows[0].trim_end(), " ✗ the turn stopped: OpenAI refused the request (400).");
    assert_eq!(rows[1].trim(), "OpenAI said: \"Invalid 'tools[0].function.name': string does not match pattern.\"");
    // a warning for another cause stays
    let mut events = vec![Ev::Warn("candidate discarded: interrupt".into())];
    let mut cache = vec![None];
    push_event(&mut events, &mut cache, Ev::Err(format!("turn failed: {}", why)));
    assert_eq!(events.len(), 2);
}

#[test]
fn refusal_parts_split_bise_and_the_provider() {
    assert_eq!(
        refusal_parts("turn failed: OpenAI refused the key (401). check it with /setup. OpenAI said: \"Incorrect key. See docs.\""),
        Some(("OpenAI refused the key (401). check it with /setup.", "OpenAI said: \"Incorrect key. See docs.\""))
    );
    assert_eq!(
        refusal_parts("turn failed: Mistral refused the request (400)."),
        Some(("Mistral refused the request (400).", ""))
    );
    assert_eq!(refusal_parts("turn failed: provider unreachable: retries exhausted"), None);
    assert_eq!(refusal_parts("compaction failed: x refused the y. z said: \"w\""), None);
}

// subscriptions (design item 5): the ChatGPT plan's four turn lines read as
// a ▲ that says what to do, matched on their openings; the limit line's
// usage page is a link; any other failure stays a ✗
#[test]
fn the_chatgpt_plan_lines_are_a_warning_and_the_rest_a_failure() {
    let lines = [
        "your ChatGPT plan's limit for bise is reached. your usage is at chatgpt.com/settings/usage, or switch model with /model.",
        "ChatGPT plan use is off for bise. turn it on in your ChatGPT settings, or pick another provider in /provider.",
        "ChatGPT couldn't check your plan's usage. try again in a moment, or switch model with /model.",
        "your ChatGPT sign-in expired. sign in again in /provider, or run bise login chatgpt.",
    ];
    for l in lines {
        let ev = parse_line(&format!("  obs: turn_done: failed: {}", l)).expect("parse");
        assert!(matches!(&ev, Ev::Warn(w) if w == l), "{l}");
        let text = rows_text(&ev_rows(&ev, 0, 200)).iter().map(|r| r.trim()).collect::<Vec<_>>().join(" ");
        assert_eq!(text, format!("▲ {}", l));
    }
    // the limit line: one link, the usage page, on its own words
    let ev = Ev::Warn(lines[0].to_string());
    let rows = ev_rows(&ev, 0, 200);
    let linked: Vec<String> = rows
        .iter()
        .flat_map(|r| r.spans.iter())
        .filter(|sp| sp.style.add_modifier.contains(ratatui::style::Modifier::UNDERLINED))
        .map(|sp| sp.content.to_string())
        .collect();
    assert_eq!(linked, vec!["chatgpt.com/settings/usage".to_string()]);
    // a reworded tail still matches; another provider's failure stays ✗
    let ev = parse_line("  obs: turn_done: failed: your ChatGPT sign-in expired. (401)").expect("parse");
    assert!(matches!(ev, Ev::Warn(_)));
    let ev = parse_line("  obs: turn_done: failed: OpenAI refused the request (400).").expect("parse");
    assert!(matches!(&ev, Ev::Err(t) if t == "turn failed: OpenAI refused the request (400)."));
    assert!(rows_text(&ev_rows(&ev, 0, 200)).join("\n").starts_with(" ✗ "));
}

// interrupt-who: a call an interrupt stopped mid-answer names who asked
// and reads as a dim ▲ stop, not a red ✗ failure
#[test]
fn an_interrupt_mid_answer_is_a_dim_stop_naming_who() {
    for (who, text) in [("main", " ▲ turn interrupted by main"), ("the user", " ▲ turn interrupted by the user")] {
        let ev = parse_line(&format!("  obs: turn_done: failed: interrupted by {}", who)).expect("parse");
        assert_eq!(rows_text(&ev_rows(&ev, 0, 100)).join("\n"), text);
    }
}
