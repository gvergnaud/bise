# mermaid

Mermaid diagrams in the thread: the agent writes Mermaid source, the view
draws it in bise's look (light and dark, bise's fonts and series colors),
with pan and zoom, the source (editable), and SVG, PNG and .mmd export.
Off by default: turn it on in the plugins page.

## Tools

| tool | for | answers |
| --- | --- | --- |
| `render_diagram {source, title?}` | the model (and the view) | draws it under the call; a syntax error is an `isError` result with the line, column, the line's text and what was expected, so the agent fixes it alone |
| `check_diagram {source}` | the model | the type, or the same error, without drawing |
| `diagram_examples {type?}` | the model | the 22 diagram types, when to use each, an example that draws |

The view (`ui://mermaid/diagram`) shows a syntax error with its lines and a
caret, "Ask the agent to fix it" (`ui/message`) and "Edit the source". An
edit the user draws reaches the agent as the view's context
(`ui/update-model-context`).

## Use cases (what to ask)

- "draw the login flow of this repo as a sequence diagram"
- "make a flowchart of how a task goes from main to landed"
- "the tables of the billing schema as an ER diagram"
- "a Gantt chart of the launch plan for the next 3 weeks"
- "a mind map of the ideas in this thread"
- "the states of an agent and what moves it between them"

## How it is built

- `server.mjs`: stdio MCP server, Node 18+, no npm dependency. It parses
  each source with the vendored mermaid run in a `node:vm` context (no
  DOM: the bundled DOMPurify gets a no-op stub there, parse only), so
  the error reaches the agent in the tool result, not only in the view.
- `vendor/mermaid.min.js`: mermaid 11.12.0 (MIT, `vendor/LICENSE-mermaid`),
  the npm package's `dist/mermaid.min.js` unchanged. It is inlined into the
  view at `resources/read`, with the gallery's view kit (`../.view-kit`).
  No CDN, no network: the view's CSP stays bise's floor. To update it,
  replace the file and run `node --test plugins/mermaid/test/`: the
  server's DOMPurify stub is pinned to this build's text.
- `views/diagram.html`: the view. Labels are SVG text (`htmlLabels:
  false`) so the PNG export can draw the SVG on a canvas.

## Check

```sh
python3 tests/fixtures/mcp-apps/check.py plugins/mermaid/
node --test plugins/mermaid/test/
```
