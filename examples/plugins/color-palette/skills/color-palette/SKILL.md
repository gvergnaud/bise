---
name: color-palette
description: Show a color palette as swatches the user can copy. Use when the user asks for colors, a palette, a theme, brand colors, or to check a color's contrast.
---

# Color palette

1. Pick 3 to 8 colors and call the plugin's `palette` tool from
   run_typescript (it is a view, not a server tool: find its exact
   dotted name with `search_tool_functions` "palette") with `name` and
   `colors: [{hex: "#1f3a5f", name: "Ink"}, ...]`.
2. The user sees swatches with their contrast on white and black (AA,
   AAA); a click copies the hex, and "Copy CSS" or "Download CSS" gives
   the CSS variables.
3. Answer in one or two lines: the idea of the palette and which pairs
   are safe for text. Do not list the hex codes again.
