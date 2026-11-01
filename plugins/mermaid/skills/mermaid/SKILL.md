---
name: mermaid
description: Draw Mermaid diagrams for the user in the thread (flowcharts, sequence, class, state, ER, Gantt, pie, mind maps, timelines, git graphs and more), with pan and zoom, the source, and SVG/PNG export. Use when the user asks for a diagram, a chart of a flow, a schema, an architecture or sequence drawing, a plan on a timeline, or when a picture of how things connect explains better than text.
---

# Mermaid diagrams

Three tools, from run_typescript:

| tool | what it does |
| --- | --- |
| `tools.mermaid.render_diagram({source, title?})` | draws the diagram under your call; on a syntax error it answers `isError` with the line, column and what was expected |
| `tools.mermaid.check_diagram({source})` | the same parse, no drawing: the diagram type or the error |
| `tools.mermaid.diagram_examples({type?})` | the 22 types, when to use each, and an example that draws |

## How

1. Write the source. Its first line names the type: `flowchart TD`
   (or `LR`), `sequenceDiagram`, `classDiagram`, `stateDiagram-v2`,
   `erDiagram`, `gantt`, `pie`, `mindmap`, `timeline`, `journey`,
   `gitGraph`, `quadrantChart`, `xychart-beta`, `sankey-beta`,
   `block-beta`, `architecture-beta`, `kanban`, `requirementDiagram`,
   `C4Context`, `packet-beta`, `radar-beta`, `treemap-beta`. Unsure of
   a type's syntax: `diagram_examples({type: "gantt"})` first.
2. Call `render_diagram` with real line breaks in `source` (no ```
   fence) and a short `title` (it names the exported file too).

   ```ts
   const r = await tools.mermaid.render_diagram({
     title: "How a task lands",
     source: `flowchart LR
     A[agent] --> G{quick gate green?}
     G -->|yes| L[sb land]
     G -->|no| F[fix] --> G`,
   });
   ```
3. An error looks like `Mermaid syntax error on line 3, column 6:
   \`C[Land\`: expected SQE, ...`. Fix that line and call
   `render_diagram` again; do not ask the user. Two failures on the same
   line: rewrite it plainer (quote labels with `"..."`, drop special
   characters).
4. The user sees the diagram; answer in one or two lines about what it
   shows. Never paste the source back in your reply: the view has a
   Source tab.

## Write diagrams that read well

- 5 to 25 nodes. More: split in several diagrams, one idea each.
- Short labels (2-5 words). Quote any label with `( ) [ ] { } : ; # "`
  or a leading digit: `A["step 1: build (fast)"]`.
- `flowchart LR` for a pipeline, `TD` for a tree or a decision.
- Do not set colors, `%%{init}%%` themes or `classDef` fills: the view
  draws in bise's look, light or dark, and a hand color breaks one of
  them.
- Sequence: declare `participant X as Name` first, in the order you
  want the columns.
- Gantt: always `dateFormat YYYY-MM-DD`; tasks as `name :id, start, 3d`
  or `name :after id, 2d`.

## What the user can do in the view

Drag to pan, ⌘ + scroll or the +/− buttons to zoom, double-click to fit,
"open in the panel" for a big one, SVG and PNG export, and a Source tab
where he can edit and redraw. When he edits, his version reaches you
with his next message (as the view's context); start from it.
"Ask the agent to fix it" on an error sends you the error and his
source.
