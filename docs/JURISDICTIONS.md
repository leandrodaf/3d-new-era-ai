# Jurisdictions: how a review answers to where a project is

Companion to [STANDARDS.md](STANDARDS.md), which describes the sources and the
figures. This document is about the other half: how the software judges a
project in Miami differently from one in São Paulo, what it took, how to add a
country, and what is deliberately not done yet.

Read STANDARDS.md first.

## Why this exists

The registry knew **who** said a thing. The rules knew **what** was said. The
numbers — `85.0`, `50.0`, `40.0` — were literals in rule bodies with a source
code as a tag beside them, so adding a jurisdiction meant forking a rule rather
than adding a row. `Tier` was a stored field documented as "Brazilian standard
or municipal code", which made the letter a property of the source instead of a
relation between the source and the place. And a room was a Portuguese word:
a plan whose rooms were called "Bedroom" tripped no electrical rule at all.

## What the model does

### The letter is computed, not stored

`Standard` carries `Authority` (whose word it is, over what territory), `Kind`
(what it is), `Status` (in force, or withdrawn in a given year) and, for the one
case nothing can compute, `tier_override`. `Standard::force(at)` derives the
tier from those against a `Place`.

- `Place { country, region, city }` — hierarchical and partial. Below the
  country, silence is not disagreement; naming a different city is.
- `Place::HOME_COUNTRY` — a project that says nothing is judged at home, not
  nowhere. The compass starts with no city, so this is the common case.
- `Compass.country` / `Compass.region`, set by `set_home(country=…, region=…)`.
  A city the registry holds implies both. A city named for one call (the
  review's `city`) replaces only the city: the country and the state the
  project declares still stand.
- `Report.place` — the place travels with the report, because whoever shows a
  citation has to know where it was weighed.

### Every number is asked for

`Figure { name, authority, source, bound, severity, note }` and
`figure(name, at)`, resolved once per review with `figures_at`. The rules ask
for every number they compare against, from room areas to the switch reach
band; what stays in a rule is its formula and its geometry. The details are in
[STANDARDS.md](STANDARDS.md#figures-the-numbers-and-who-demands-them). Three
things in it are worth knowing before changing it:

- **Severity lives on the figure.** The tier is a ceiling and most requirements
  sit far below it; deriving the severity from `force()` would turn dozens of
  tips into errors.
- **A source is optional.** Eight clearances and a few limits are practice that
  no publication states. They cite nothing and nothing caps them.
- **Where nothing carries, the home country's national rows still inform**, at
  the letter they have there. A city's or a state's rows never travel, and a
  foreign row never speaks at home.

### One policy for how much a finding may claim

`standards::weigh(severity, code, place)` is the ladder: the tier caps, a
survey raises nothing, a figure not confirmed at the source may warn but never
accuse. The review applies it to every finding it makes. The electrical,
plumbing and guard checks apply what a border takes from their sources
(`electrical::weighed`): each of them sets its severities with its source in
mind at home, so at home nothing moves, and in Texas NBR 5410 warns instead of
accusing. The review then counts an installation project's finding one step
lighter (`imported`), and a guard's as it is.

A finding's discipline — the score it counts in, and whether `ReviewScope`
can leave it out — comes from its rule, never from the code it cites.

### A room is what its name means

`newera_core::vocabulary` holds the words, one table per language (Portuguese
and English), each pointing at a `Mention`. A name gives a *set* of mentions —
"Sala e cozinha" is a living room to the review and a kitchen to NBR 5410 — and
every check keeps its own precedence over the set. A declared `RoomUse` maps
straight to mentions, in no language.

### A finding is a template and its data

`newera_core::text::Text` keeps the template, the data and the Portuguese it
renders to, built with `say!` instead of `format!`. The Portuguese is canonical
and byte for byte what it was; it is what findings are merged and accepted by,
and what the pre-`Rule` acceptance keys were built from. A word inside a
sentence — a side, a room's use, a fixture — is a `Text` of its own and is
translated with it. `text::ENGLISH` covers every sentence; the window shows
findings in English when it speaks anything but Portuguese.

## Adding a country

The United States is the worked example: `crates/newera-core/src/standards/us.rs`.

1. **Sources.** A `STANDARDS` slice of the codes that oblige a home there, each
   with its `Authority` (usually `Country`; `Region` where a state adopts its
   own), `Kind` and edition. Add it to `sources()`.
2. **Figures.** A `FIGURES` slice, one row per number, under the names the
   rules already ask for, converted to cm and with the article in the note. Add
   it to `figures()`. A name the pack leaves out still hears the home standard
   as a reference; a name the home pack only knows at city or state level goes
   quiet there, which is right — and the applicability test says which.
3. **Per-use names.** If the code draws a line the home pack does not — the IRC
   lets bathrooms and laundries be lower than hallways — the rule asks the more
   specific name first (`room.<use>.min_ceiling`, then `room.min_ceiling`).
   The home pack gets rows under the new names; nothing at home moves.
4. **Sentences that name a source.** A message that says who demands a number
   ("o Código Sanitário de SP pede") has to say something else when the number
   comes from elsewhere: the figure's note.
5. **Words.** Room names in the country's language go into
   `vocabulary::Language`; the guard check's piece names into `guard_of`.
6. **Tests.** `IN_BRAZIL` in `standards.rs` lists what every source is worth at
   home, so a new source says what it is worth there. Then `make golden`:
   `golden-br.txt` must not move, and `golden-abroad.txt` moves on purpose and
   is read.

## Not done, on purpose

- **State adoption.** The IRC and the NEC are model codes; a state or a city
  adopts an edition, sometimes amended. They stand at the country's level,
  which is right for the numbers in the pack and not a claim that every town
  enforces them.
- **The installation projects abroad.** NBR 5410 against the NEC and NBR 8160
  against the IPC is a design problem — circuits by kind, VA per room, 120 V —
  not a translation. The electrical and plumbing checks still speak the
  Brazilian standards; abroad their findings arrive as references, which is
  honest until then.
- **Egress.** IRC R310 (a sleeping room's emergency escape opening) and R311.2
  (the egress door) have no rule to answer to.
- **A117.1-2017.** It widened the turning space and the clear floor space for
  new buildings (67 in, 30 × 52 in); the pack holds the 2009 edition the IBC
  2015 and 2018 reference.
- **Spanish and French findings.** The interface speaks them; findings fall
  back to English there. A language is one more table beside `text::ENGLISH`,
  held whole by the same tests.
- **Numbers inside a sentence** keep Portuguese formatting (`1,60 m`,
  `12,5`). A translation says the sentence in another language; it does not
  reformat its data.

## Rules the next change must not break

- **`Rule::id()` and the key shape `rule_id:place_id[:about]` never change.**
  They are data in saved projects.
- **The Portuguese sentence is data too.** `legacy_key_of` builds a pre-`Rule`
  acceptance key from the cited code, the place and the first four words of
  the Portuguese message. `tests/fixtures/legacy-keys.txt` holds the corpus's
  keys; rewording a template moves them, and that has to be a decision.
- **A registry code is almost persisted**, for the same reason: renaming or
  retiring one drops legacy acceptances. `irc2024` kept its name when the IRC's
  planning chapter got an entry of its own.
- **`Facet` keys stay the literals `front` / `left` / `right`**, and `{a}+{b}`
  for pairs, or `clearance:f12:front` acceptances die.
- **Applicability destroys data if it is wrong.** "No figure resolves, so the
  rule does not apply" feeds `orphaned()`, and `accept(prune=true)` then
  *deletes* those acceptances. `every_figure_resolves_everywhere_unless_it_is_local`
  is the table of figure × place that prevents it.
- **Resolve once per review, never per finding.** `fix_creates` re-reviews per
  finding that carries a fix, `orphaned()` reviews once per storey, and every
  MCP edit reply reviews twice more.
- **`make golden`** runs the review over the corpus, optimised, and CI runs it
  in its own job. `golden-br.txt` does not move unless a change means to move a
  finding in Brazil, and says so.
- `crates/newera-mcp/tests/fixtures/tool-surface.json` locks all 59 tools;
  regenerate it deliberately with the `#[ignore]`d `update_tool_surface_snapshot`.

## Two things found on the way

- A debug print left in a test, `eprintln!("DBGFIX …")`, is gone.
- `guard:*` acceptances were thought to have no `orphaned()` to answer for
  them. They do: the review brings the guard findings in, and its `orphaned()`
  lists a guard acceptance whose finding is gone —
  `a_guard_acceptance_is_orphaned_when_its_finding_is_gone` holds it.
