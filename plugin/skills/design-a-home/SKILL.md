---
name: design-a-home
description: Design or furnish a room or a whole home in 3D New Era AI — walls, rooms, doors, windows, furniture, lighting, and a check against the people who live there. Use when the user asks to draw, lay out, furnish or improve a floor plan.
---

# Design a home in 3D New Era AI

The `newera` MCP server edits the plan in the 3D New Era AI window when it is open
(the user sees every change and can undo it with Ctrl+Z), or a project of its own when
it is not. Units are centimeters; the plan's x grows right and y grows down.

1. **Read first.** `get_home` with `detail=summary` gives counts, bounds and room areas.
   Read more only where you work: `room=`, `rect=` or `ids=`.
2. **Walls and rooms in one call.** `create` takes `walls` (polylines, `closed=true` for a
   loop), `rooms` (`at=[x,y]` detects the room from the walls around it), dimensions and
   labels. Use the ids the reply returns; never guess the next one.
3. **Doors, windows, furniture.** Find items with `catalog` (`q="cama casal"`), then `place`
   them: doors and windows with `wall=<id>` and `along`, furniture with `at` and
   `facing=+x|-x|+y|-y|<id>`, or `wall=<id>` to put its back on a wall.
4. **Who lives there.** Ask, then keep it: `set_home(people={occupants, children, elderly,
   wheelchair})`. Every review scores for them from then on.
5. **Check.** `check_layout` for clashes, blocked doors and pieces turned the wrong way;
   `ergonomics` for circulation, beds, kitchen and accessibility. A finding with a `fix`
   is a checked change: apply it and review again. When a finding is right as drawn,
   `accept` it with the reason.
6. **Light.** `lighting` rates every room against NBR ISO/CIE 8995-1; `fill_lighting`
   places the fixtures a room needs.
7. **Show it.** `show_plan` puts an interactive plan in the chat where the client can show
   one; `render_plan` is the picture for you to look at; `render_photo` (quality `draft`
   first) is the photo for the user.

Reads never change the plan; each change is one undoable step. Before a large change,
`checkpoint` with a label lets you come back to it.
