---
name: kanban
description: Keep a simple kanban board (To do, Doing, Done) with the user. Use when the user wants to track tasks on a board, plan work in cards, or asks what is in progress.
---

# Kanban

1. Call `tools.kanban.show_board` from run_typescript to show the board.
   The user can open it in the panel, drag cards and add some.
2. Change it with `tools.kanban.add_card` (`title`, `note`, `column`:
   todo, doing or done), `tools.kanban.move_card` (`id`, `column`,
   `position`) and `tools.kanban.remove_card` (`id`). The board the user
   sees updates by itself within 3 s: no need to show it again.
3. Every tool answers the whole board as text (`#id title` per column):
   read the ids there, never guess them.
4. A message from the view ("Split the card #4 …") is the user's
   request: add the smaller cards, then remove the big one.
