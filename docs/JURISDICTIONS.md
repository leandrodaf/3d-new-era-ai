# Jurisdictions: what is done, and what is left

Companion to [STANDARDS.md](STANDARDS.md), which describes the model as it now
stands. This document is the other half: the work still ahead to make the
software judge a project in Miami or Munich the way it judges one in São Paulo,
and where the work stopped.

It is a handoff, not a specification. Read STANDARDS.md first.

## Why this exists

The registry knew **who** said a thing. The rules knew **what** was said. The
numbers — `85.0`, `50.0`, `40.0` — were literals in rule bodies with a source
code as a tag beside them, so adding a jurisdiction meant forking a rule rather
than adding a row. `Tier` was a stored field documented as "Brazilian standard
or municipal code", which made the letter a property of the source instead of a
relation between the source and the place.

Three things follow from that, and only the first is finished.

## Done

### The letter is computed, not stored

`Standard` carries `Authority` (whose word it is, over what territory), `Kind`
(what it is), `Status` (in force, or withdrawn in a given year) and, for the one
case nothing can compute, `tier_override`. `Standard::force(at)` derives the
tier from those against a `Place`. The stored `tier` field is gone.

- `Place { country, region, city }` — hierarchical and partial. Below the
  country, silence is not disagreement; naming a different city is.
- `Place::HOME_COUNTRY` — a project that says nothing is judged at home, not
  nowhere. The compass starts with no city, so this is the common case, and
  demoting every Brazilian standard there would have turned a gas appliance
  with no permanent opening from an error into a warning.
- `Report.place` — the place travels with the report, because whoever shows a
  citation has to know where it was weighed.
- `Compass.country` / `Compass.region`, set by `set_home(country=…, region=…)`.
  A city the registry holds implies both.
- The desktop "Código de obras" picker writes to the compass, so choosing a
  building code survives saving. It used to write to a field stripped before
  persisting.

Pinned by `every_source_is_worth_in_brazil_what_it_was_worth_before_force_was_computed`
(41 entries × 5 Brazilian ways of naming a place) and by
`the_same_source_changes_force_when_the_place_changes`.

### The first figures left the rule bodies

`Figure { name, authority, source, bound, note }` and `figure(name, at)`: a
number with an authority, resolved against the place, strictest wins.
`Review::rooms` asks for `room.<use>.min_area` and `room.<use>.min_side`
instead of deciding them, which is what stops São Paulo's decree 57.776 from
measuring a project in Curitiba.

The authority lives on the figure, not only on its `source`, because one
registry entry stands for several authorities — `coe-municipal` covers every
municipal code — and because keeping the cited code stable is what keeps
acceptances written under it working.

## Where the work stopped

The last commit on this branch carries the `Figure` table and the `rooms`
migration. `cargo test -p newera-core -p newera-ergonomics` is green (205 + 38).
**The full workspace suite and clippy were last run green before that commit**,
not after it — re-run both first:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Nothing else was started. The sections below are the plan, in order.

## Left to do

### 1. The rest of the figures (`newera-ergonomics`)

One rule group per change, each proving no finding moves in Brazil.

| Group | Where | What is still a literal |
|---|---|---|
| `clearances` | lib.rs, the `match u.what` arm | ~30 minimums across 16 arms: 85/150 in front of kitchen units, 50/90 beside a bed, 40/120 at a toilet and basin, 75 at a table, 60 at a shower, 70 at a dresser, 90 around an island |
| `kitchen` | lib.rs | NKBA's 792/274/122, the Blum counter height, Alexander's 366/122/305, the five zones, 350 cm per socket, the Caixa 180 |
| `gaps`, `reach` | lib.rs | 60 between beds, 85.5 and 46 accessible tops, the 40–100 and 60–100 reach bands |
| `occupancy` | lib.rs | 3 per bedroom, 5 per bathroom, 80 + 40 cm of wardrobe per person |
| ceiling height | `rooms`, after the figures | 230 / 250 cm |

**`Bound` covers about 20 of the 54 rules and that is fine.** The rest are
formulas, not limits: a work triangle, a counter height from stature, Alexander's
three coupled constraints, sockets per metre of perimeter. The rule is *the
formula stays in code; its parameters become figures*. Do not grow `Bound` into
a little language.

Four axes a figure cannot yet express, all of which already exist in the rule
bodies and will be needed by `clearances`:

- **the band on the face** — `need(side, min, span, …)` takes a span, `(0.25,1.0)`
  beside a bed, `(0.2,0.8)` at seats, and a computed one from `blind_left` /
  `blind_right` on a corner cabinet;
- **the counterparty** — hood against stove, sink against stove, bed against bed;
- **a quantifier** — `Bed(1)` needs one side free, `Bed(2)` needs both, and that
  is a condition on the subject's own parameter, not on the profile;
- **an aggregate selector** — median for the counter height, min for the shortest
  run, sum for the total, max pairwise for the spread.

### 2. Severity belongs on the figure

`Severity::Erro` is `#[default]` and sorts **first**, so
`severity.max(Severity::for_tier(tier))` in `push_ref` picks the *weaker* of the
two: the tier is a ceiling, and the severity each rule declares is what
actually decides. It is usually far below the ceiling — `RoomArea`,
`WindowArea`, `MissingFixtures` and `GasCookingInBedroom` all declare `Dica`
against tier A sources.

So a figure that carries a requirement must carry its severity too. Deriving it
from `force()` would turn dozens of tips into errors at 12 score points each.
Prove the shape on `Use::Toilet` and `Use::Basin` alone before migrating a
whole group.

Related: eight clearances have **no source at all** and rely on `push` applying
no cap — the crib's 50 cm, the wardrobe's leaf + 10, the dresser's 70, the
island's 90, the shower's 60, the desk's 75, `WallCabinetLow`'s 135 and
`SinkNextToStove`'s 60, four of them at `Alerta`. Either a figure's source is
optional, or there is a `Kind::Practice` that is explicitly uncapped. Requiring
a source of all eight demotes four findings.

### 3. A room stops being a Portuguese word

This blocks every foreign pack and is invisible until one is tried: a project
whose rooms are called "Bedroom" and "Kitchen" triggers **no rule at all** — not
because nothing applies, but because nothing recognises the room.

Five independent keyword lists to collapse onto `RoomUse`
(`newera-core/src/elements.rs`), with one name table per language:

- `room_use_by_name`, `newera-ergonomics/src/scene.rs`
- `Wet::room_class`, `newera-core/src/electrical.rs`
- `Fixture::of`, `newera-core/src/plumbing.rs`
- `guard_of`, `newera-core/src/guard.rs`
- `recommended_lux`, `newera-core/src/lighting.rs`

`RoomUse::semantic_name` returns Portuguese today, and every classifier greps
those strings, so it is the pivot to fix first.

### 4. The other disciplines join the model

`electrical.rs`, `plumbing.rs`, `lighting.rs` and `guard.rs` never call
`standards::` at all. They cite `source: &'static str` as free strings — 29, 21,
0 and 7 of them — on their own `Finding` type with a duplicate `Severity`
(electrical.rs), and **no tier cap of any kind**. Ergonomics demotes imported
electrical and plumbing findings one step by hand while the discipline tools
show them at full severity, and guard findings are not demoted at all: three
policies over one shape.

Two cheap, independent first steps, worth doing before any number moves:

1. a test that every cited code resolves in the registry — today an unknown code
   only `debug_assert!`s in `push_ref`, and the disciplines have nothing;
2. unify the duplicate `Severity`.

`lighting.rs` is the worst case: `recommended_lux` carries no source, and the
tool cites `["nbr5413", "nbr8995"]` regardless of which value was used.

Only then the numbers, because NBR 5410 against the NEC and NBR 8160 against
the IPC is a design problem — TUG/TUE, VA per room, 127 V — not a translation.

### 5. Findings become template and data

`i18n.rs` translates the interface; finding messages are Portuguese strings
built inside the rules and printed raw. Needed before *selling* a foreign pack,
not before modelling one.

**Do this carefully.** `legacy_key_of` builds a pre-`Rule` acceptance key out of
the cited code and *the first four words of the message*, and projects saved
before rules were named still carry those keys. Rewording every message drops
every legacy acceptance silently. Snapshot the legacy keys of the current corpus
into a fixture first.

### 6. `us.rs` — and it is not a data-only change

`Finding::discipline()` matches `reference == Some("nbr5410")`. An American
electrical requirement citing `nec2023` classifies as `"architecture"`, counts
in the architecture score and cannot be excluded by
`ReviewScope { electrical: false }`. Discipline has to come from the rule, not
from a code string. That is a prerequisite, not a detail.

## Rules the refactor must not break

- **`Rule::id()` and the key shape `rule_id:place_id[:about]` never change.**
  They are data in saved projects.
- **A registry code is almost persisted too**, through `legacy_key_of`:
  renaming or retiring one drops legacy acceptances.
- **`Facet` keys must stay the literals `front` / `left` / `right`**, and
  `{a}+{b}` for pairs, or `clearance:f12:front` acceptances die.
- **Applicability destroys data if it is wrong.** "No figure resolves, so the
  rule does not apply" feeds `orphaned()`, and `accept(prune=true)` then
  *deletes* those acceptances. A missing row is not a silent no-op. The
  table test of rule × place is what prevents it.
- **Resolve once per review, never per finding.** `fix_creates` clones the
  document and re-reviews per finding that carries a fix, `orphaned()` reviews
  once per storey, and every MCP edit reply reviews twice more.
- `crates/newera-mcp/tests/fixtures/tool-surface.json` locks all 59 tools;
  regenerate it deliberately with the `#[ignore]`d `update_tool_surface_snapshot`.
- `const fn s()` sits at eight parameters with an `#[allow(clippy::too_many_arguments)]`;
  CI runs clippy with `-D warnings`.

## Two defects found on the way, still open

- `guard:*` finding keys are covered by no `orphaned()` implementation, so those
  acceptances can never be pruned.
- A debug print is left in a test: `eprintln!("DBGFIX …")` in
  `crates/newera-ergonomics/src/lib.rs`.
