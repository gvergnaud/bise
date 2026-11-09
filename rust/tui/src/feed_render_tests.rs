//! Wire line -> feed rows, end to end: parsing, merging, code blocks,
//! wrapping and display widths.

use super::*;
use crate::code::*;
use crate::editor::layout_input;
use unicode_width::UnicodeWidthStr;

#[test]
fn provider_retry_and_restart_render() {
    match parse_line("  obs: provider_retry: 2/10 · provider 529 (transient) · retry in 4s") {
        // H9: a warning that carries its data (one updating row per run)
        Some(Ev::Retry { text: t, .. }) => assert_eq!(
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
    assert_eq!(think, "a\\nb", "each span without its signature line");
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
        Ev::You(prose.clone().into(), Mark::Read, false, None),
        Ev::Assistant(prose.clone()),
        Ev::AgentMsg { from: "docs".into(), to: "main".into(), text: prose.clone(), level: 3, id: String::new(), parts: Vec::new(), open: false, fold: false },
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
    Ev::AgentMsg { from: from.into(), to: String::new(), text: text.into(), level: 3, id: "m_3".into(), parts: Vec::new(), open: false, fold: false }
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
    // the thinking section opens
    assert!(toggle_event(&mut app.events, &mut app.cache, 1));
    assert!(matches!(app.events[1], Ev::Thinking { open: true, .. }));
    // an output
    ensure_rows(&app.events, &mut app.cache, 0, false, 80, 0);
    assert!(toggle_event(&mut app.events, &mut app.cache, 0));
    assert_eq!(expanded(&app), vec![true, false, false]);
    assert!(app.cache[0].is_none(), "its rows rebuild");
    // nothing behind a notice or a tool without output
    assert!(!toggle_event(&mut app.events, &mut app.cache, 4));
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
        Ev::AgentMsg { from: "main".into(), to: String::new(), text: brief.into(), level: 3, id: "m_1".into(), parts: Vec::new(), open, fold: false },
        Ev::Thinking { ms: 14_000, text: "safari drops the session cookie on the redirect. SameSite=None needs\nSecure, and the dev server sets it without. check session.ts first.".into(), open },
        Ev::Tool(b),
        Ev::Tool(t),
        Ev::Sub { name: "github.search_issues".into(), ok: true, preview: "2 items".into(), account: None },
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
    let lines = ev_rows(&Ev::Card { card: crate::wire::CardParts::of_line("#2 question @docs : v1 or v2 for the api docs?"), closed: String::new() }, 0, 100);
    let rows = rows_text(&lines);
    assert_eq!(rows, vec![" ┃ ? docs needs you".to_string(), " ┃ v1 or v2 for the api docs?".to_string()]);
    let accent = Some(crate::theme::accent());
    assert_eq!(lines[0].spans[0].style.fg, accent);
    assert_eq!(lines[0].spans[1].style.fg, accent);
    assert!(lines[0].spans[1].style.add_modifier.contains(Modifier::BOLD));
    assert_eq!(lines[1].spans[1].style.fg, Some(crate::theme::text()));
    let blocked = rows_text(&ev_rows(&Ev::Card { card: crate::wire::CardParts::of_line("#4 blocked @api-v2 : the schema file isn't in the repo"), closed: String::new() }, 0, 100));
    assert_eq!(blocked[0], " ┃ ? api-v2 needs you");
    // drop-card (designer m_37336): a card with its own title says it, as the inbox does
    let drop = rows_text(&ev_rows(&Ev::Card { card: crate::wire::CardParts::of_line("#5 drop @gone : main suggests it. nothing of its work is lost."), closed: String::new() }, 0, 100));
    assert_eq!(drop[0], " ┃ ? archive gone?");
    let done = rows_text(&ev_rows(&Ev::Card { card: crate::wire::CardParts::of_line("#3 done @bench : p95 at 180 ms"), closed: String::new() }, 0, 100));
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
    let card = |t: &str| Ev::Card { card: crate::wire::CardParts::of_line(t), closed: String::new() };
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
        ("handed to main", "handed to main"),
    ] {
        let ev = Ev::Card { card: crate::wire::CardParts::of_line("#5 drop @x : y"), closed: res.into() };
        // a drop card says its own title (drop-card, designer m_37336)
        assert_eq!(rows_text(&ev_rows(&ev, 0, 100))[0], format!(" ┃ ? archive x? · {}", word));
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
    let sub = |name: &str, ok: bool, preview: &str| Ev::Sub { name: name.into(), ok, preview: preview.into(), account: None };
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

/// The compaction's summary folds like the other rows: ctrl+o opens it
/// (and closes it again), and open it shows every word of the whole
/// summary as markdown (its heading without the `##`, its list), wrapped
/// to the width, never cut.
#[test]
fn ctrl_o_opens_the_whole_compaction_summary_as_markdown() {
    let tail = "the cold run is slow because the index is rebuilt at every start, keep it warm";
    let summary = ["## What was done", "- profiled the **hub**", &format!("- {tail}")].join(&char::from(10).to_string());
    let mut events = vec![Ev::Compacted { text: summary, open: false }];
    let mut cache: Vec<Option<EventRows>> = vec![None];
    assert_eq!(feed_text(&events, 40), [" ≡ summary ▸"]);
    crate::feed::set_everything(&mut events, &mut cache, true);
    let rows = feed_text(&events, 40);
    assert_eq!(rows[0], " ≡ summary ▾");
    assert!(rows.iter().any(|r| r.ends_with("│ What was done")), "{rows:#?}");
    assert!(!rows.iter().any(|r| r.contains("##") || r.contains("**")), "{rows:#?}");
    let said: Vec<&str> = rows[1..].iter().flat_map(|r| r.split_whitespace()).filter(|w| *w != "│").collect();
    let said = said.join(" ");
    assert!(said.contains(&tail.split_whitespace().collect::<Vec<_>>().join(" ")), "{rows:#?}");
    assert!(rows.iter().all(|r| UnicodeWidthStr::width(r.as_str()) <= 40), "{rows:#?}");
    crate::feed::set_everything(&mut events, &mut cache, false);
    assert_eq!(feed_text(&events, 40), [" ≡ summary ▸"]);
}

/// A list item longer than its row, in a reply and in an open summary,
/// at 80 and 150 columns: its wrapped rows hang under its text (4
/// columns in, after `  - ` or `  1. `'s mark), never back at the edge
/// (designer m_41685).
#[test]
fn a_wrapped_list_item_hangs_under_its_text() {
    let long = "the cold run is slow because the index is rebuilt at every start of the hub and nobody keeps it warm between two runs of the gate, so every agent pays it again and again, all day long, on every machine";
    let md = [format!("- {long}"), format!("1. {long}")].join(&char::from(10).to_string());
    for width in [80, 150] {
        for (what, ev) in [("reply", Ev::Assistant(md.clone())), ("summary", Ev::Compacted { text: md.clone(), open: true })] {
            let rows = feed_text(&[ev], width);
            if let Some(f) = std::env::var_os("HANG_DUMP") {
                let _ = std::fs::write(format!("{}-{what}-{width}.txt", f.to_string_lossy()), rows.join(&char::from(10).to_string()));
            }
            // the rows of the item's text, from its mark to the next item or the end
            for mark in ["- ", "1. "] {
                let at = rows.iter().position(|r| r.contains(&format!("  {mark}the cold run"))).unwrap_or_else(|| panic!("{what} {width}: {rows:#?}"));
                let col = rows[at].find("the cold").unwrap();
                let next: Vec<&String> = rows[at + 1..].iter().take_while(|r| !r.contains("the cold run")).filter(|r| !r.trim().is_empty() && r.trim() != "│").collect();
                assert!(!next.is_empty(), "{what} {width}: the item wraps: {rows:#?}");
                for r in next {
                    let text_at = r.char_indices().find(|(i, c)| *i >= 1 && !c.is_whitespace() && *c != '│').map(|(i, _)| i).unwrap();
                    assert_eq!(text_at, col, "{what} {width}: {r:?} hangs under the text: {rows:#?}");
                }
            }
        }
    }
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
    assert_eq!(row(Ev::Sub { name: "gh.x".into(), ok: false, preview: "404".into(), account: None }), "   ↳ gh.x ✗ 404");
    // the account a sub-call used follows its name (multi-accounts piece 5)
    assert_eq!(row(Ev::Sub { name: "gmail.search".into(), ok: true, preview: "1".into(), account: Some("perso".into()) }), "   ↳ gmail.search · perso ✓");
    assert!(row(Ev::You("hi".into(), Mark::Read, false, None)).starts_with("│  hi"));
    // a tool the turn abandoned says so in English
    let mut events = vec![Ev::Tool(ToolData::bare(9, ToolState::Run))];
    let mut cache = vec![None];
    push_event(&mut events, &mut cache, Ev::TurnDone);
    assert!(matches!(&events[0], Ev::Tool(td) if td.result.as_ref().is_some_and(|r| r.1 == "interrupted")));
}

// ---- BISE-14: the three levels, folding, time marks (book §9, §10) ----

fn l3(from: &str, to: &str, text: &str) -> Ev {
    Ev::AgentMsg { from: from.into(), to: to.into(), text: text.into(), level: 3, id: String::new(), parts: Vec::new(), open: false, fold: false }
}

fn to_you(from: &str, text: &str) -> Ev {
    Ev::AgentMsg { from: from.into(), to: "you".into(), text: text.into(), level: 2, id: String::new(), parts: Vec::new(), open: false, fold: false }
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

/// His messages and their marks, as the feed holds them (the marks and the folds read them).
fn yours(events: &[Ev]) -> Vec<(String, Mark, Option<u64>)> {
    events.iter().filter_map(|e| if let Ev::You(t, m, _, id) = e { Some((t.text.clone(), *m, *id)) } else { None }).collect()
}

mod folds;
mod levels;
mod marks;
