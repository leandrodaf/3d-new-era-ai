# Designing MCP tools

The tool list is the one prompt every agent reads before it does anything for the
user. Each tool it carries costs tokens on every conversation, and each tool that
reads like another is a choice the model makes by luck. This page says how a tool
is added or changed so the surface grows without getting worse; every rule here
has a test in [`crates/newera-mcp/src/surface_rules.rs`](../crates/newera-mcp/src/surface_rules.rs)
or [`hints.rs`](../crates/newera-mcp/src/hints.rs), so CI says when one is broken.

## Before adding a tool

Ask whether it is an `action` of a tool that already exists. A new tool is right when
it is a new *intent* an agent would search for ("check the plumbing", "trace a scan"),
not a new endpoint of an intent already there. Clients defer tool definitions and let
the model search them, so a tool also has to be *findable*: add the requests it answers
to [`tool-search.json`](../crates/newera-mcp/tests/fixtures/tool-search.json).

## Names

- `snake_case`, at most 32 characters.
- A read is the noun of its domain (`cameras`, `electrical`, `levels`).
- The write that changes it is `edit_<noun>`, with an `action` (`edit_cameras`). Reads
  and writes never share a tool: the client runs a read without asking and asks before
  every write, which only works while the hint is true for every action of the tool.
- Verbs name operations on elements (`create`, `update`, `delete`, `move`, `place`,
  `arrange`, `accept`). A read never starts with a write verb.
- Enforced by `names_say_what_a_tool_does`. A renamed tool's old name goes in
  `RETIRED`, and `descriptions_name_tools_that_exist` then finds it anywhere it was left.

## Descriptions

A description is, in this order:

1. what the tool does, in one sentence, with the words a user would use;
2. when to use it rather than its neighbours;
3. the shape of the reply (rows, fields);
4. the tools next to it, by name.

It is not a manual. The rules behind a review (what a standard demands, how a load is
sized) belong in the review's answer: each finding carries its `msg` and `src`, and the
standards resolve in `sources`. A description does not give orders to the model or
advertise (`descriptions_neither_order_nor_sell`).

Arguments are described in a line each. A shared type (`$defs`) is described in at most
300 bytes (`shared_definitions_are_described_in_a_line`): a longer doc comment above a
type is usually a function's, landed there by accident and paid for in every tool that
uses the type.

## Arguments

- An argument that picks one of a few things (`action`, `kind`, `view`, `format`,
  `what`, `mode`, `quality`) is an enum in the schema — `#[schemars(extend("enum" = [...]))]`
  on the field (`choices_are_listed_in_the_schema`). Its description then says what the
  values mean or which is the default, not the list again.
- Every call is checked against the tool's own schema before it runs
  ([`src/args.rs`](../crates/newera-mcp/src/args.rs)), so no struct can forget to: an
  argument nobody declared is refused by name, with what the tool takes instead
  (`unknown_arguments_are_refused_by_name`). A misspelled `dry` would otherwise write.
- An argument that belongs to some actions says so at the start of its description —
  ``For `route`: …``, ``For `cable` and `route`: …``, or `rotate/mirror: …` — and the
  same check refuses it with any other action, instead of answering as if it had been used.
- A refusal is a tool result with `isError: true`, not a protocol error: the agent reads
  why and tries again. The reason says what to do next — the valid choices, and which
  tool reads what a write was asked to read. A value with two shapes (`dry`, `facing`)
  says both when it gets neither.

## Hints and output

Every tool has a row in `HINTS` (`hints.rs`): its title for people, and whether it
reads, adds or changes. A read must change nothing (`reads_change_nothing`). Every tool
has an output schema in `output.rs`, and every answer fits it
(`every_answer_fits_its_schema`).

## Budget

[`tool-budget.json`](../crates/newera-mcp/tests/fixtures/tool-budget.json) records what
each tool costs in bytes (description, input schema, output schema) and what the whole
`tools/list` costs. `every_tool_fits_its_budget` fails when anything grows. Shrinking is
free; lock the gain in with:

```sh
cargo test -p newera-mcp update_tool_budget -- --ignored
```

Growing on purpose is the same command, reviewed in the diff. It refuses a description
over 1,200 bytes or an input schema over 6 KB unless the tool was already there and did
not grow: those tools have to shrink, not be written down. A new tool fits the caps.

## Search

`a_tool_search_finds_the_right_tool` indexes the surface the way a client's tool search
does (BM25 over names, descriptions and arguments) and runs the queries in
`tool-search.json`. The share whose tool ranks in the first three and first five has a
floor that only goes up. Run it with `--nocapture` to see the misses.

`no_two_tools_read_alike_without_a_reason` compares every pair of descriptions. A pair
that reads alike is listed in `alike` with the reason it has to be two tools; the usual
one is a read and its write.

## Checklist for a tool change

- [ ] The intent is new, or the change is an `action` of an existing tool.
- [ ] Name, `HINTS` row and output schema follow the rules above.
- [ ] Description in the four parts; no manual, no orders.
- [ ] Choices are enums; action-scoped arguments say their action; errors say what to do next.
- [ ] Queries for it in `tool-search.json`; the search floor still holds.
- [ ] `tool-surface.json` and `tool-budget.json` regenerated and the diff reviewed.
- [ ] Skills in `plugin/skills/` and the docs name the tool correctly.
