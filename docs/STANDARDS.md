# Standards and sources

Where the numbers come from when the software says a kitchen is wrong.

This is the project's bibliography: the standards, design doctrine and research
behind each rule, with edition and year. It is not decorative. The registry
lives in code, in [`crates/newera-core/src/standards.rs`](../crates/newera-core/src/standards.rs)
and one file per foreign country beside it
([`standards/us.rs`](../crates/newera-core/src/standards/us.rs)), and every
finding carries the short code of the source it stands on. Each row below
matches an entry there.

## The reliability ladder

Sources do not weigh the same, and treating them as equal is how you end up
with measurements nobody owns. The tier decides how strongly a finding may
accuse. It is `Tier` in the code, and the severity cap is one function,
`standards::weigh`, not left to whoever writes the rule.

The tier is a **ceiling**, not the severity. `Severity::Erro` sorts first, so
`severity.max(Severity::ceiling(tier))` keeps the weaker of the two, and most
requirements sit well below what their source could claim: a minimum area in a
municipal code is a tip. The severity a requirement makes when it is missed is
written beside its number (see [Figures](#figures-the-numbers-and-who-demands-them)),
and the tier only ever lowers it.

| Tier | What it is | Severity cap |
|---|---|---|
| **A · binding** | A law, standard or contractual specification that carries where the project is. Breaking it is a legal or safety problem. | `Erro` (error) |
| **B · reference** | The same kind of source from somewhere else, or an association's guideline. Good engineering, no legal force here. | `Alerta` (warning) |
| **C · doctrine** | Architects and manufacturers, and standards since withdrawn. What makes a kitchen good, not what makes it legal. | `Dica` (tip) |
| **D · measured** | Laboratory research, and a manufacturer's data about its own product. | `Alerta` (warning) |
| **E · descriptive** | Market surveys. What people do, never what they should do. | no findings |

### The letter is not a property of the source

A standard is not the same kind of claim everywhere. ABNT NBR 9050 obliges in
Brazil and informs in Florida; the IRC does the reverse. So the tier is not
stored on an entry — it is computed, by `Standard::force(at)`, from three
things that *are* stored, against the place the review is being conducted at:

| Field | What it answers |
|---|---|
| `Authority` | Whose word it is, and over what territory: `Global`, a `Bloc` like the EU, a `Country`, a `Region` (the level that adopts a building code in the United States), or a `City`. |
| `Kind` | What it is: `Law`, `Standard`, `Contract` (binding by contract — a bank's specification for the units it finances, a utility's spec for the connection it will accept), `Guideline`, `Doctrine`, `Manufacturer`, `Research`, `Survey`. |
| `Status` | `InForce`, or `Withdrawn(year)` — which keeps advising and stops obliging. |

Only `Law`, `Standard` and `Contract` can reach A, and only where their
authority carries; everything else is what it is wherever it is read. Below the
country, silence is not disagreement: a project that names Brazil and no state
is still judged by a state's code, and between the two the stricter one wins.
Naming a *different* city is disagreement — which is how a municipal decree
stops following a project to the next town.

A project that says nothing about where it is gets the home jurisdiction
(`Place::HOME_COUNTRY`), not nowhere. The compass starts with no city, so that
is the common case, and letting every Brazilian standard fall to a reference
there would quietly turn a gas appliance with no permanent opening from an
error into a warning.

One entry says out loud that it cannot be computed. NBR ISO/CIE 8995-1 is in
force and Brazilian, so the computation would have it oblige — but it governs
workplaces, and a home is not one. It carries a `tier_override` with the reason
in a comment beside it. That field exists for scope, and reaching for it to make
a letter come out right is how the taxonomy rots.

The place a report was weighed at travels with it, as `Report.place`: whoever
shows a citation has to know where it was judged, or the letter means nothing.
`set_home(country=…, region=…, city=…)` is what sets it, and a city the registry
holds implies its state and country.

Besides the tier, every entry carries a `Confidence`. `ConfirmBeforeUse` marks
what was not checked against the primary publication: it may advise, never
accuse. That is why a 170 cm kitchen comes out as a warning against the Caixa
specification, not as an error, even though the rule asks for one.

## Honesty rules that count as code rules

1. **Every measurement has an owner.** A number goes into the figure table with
   the authority that demands it, never into a constant in a rule. Common
   practice that nobody publishes says so — `source: None` — instead of
   borrowing a source it does not have.
2. **A cited standard needs edition and year.** "According to ABNT" is not a
   citation. NBR 13103 changed in 2024, NBR 15575 in 2021, and NBR 9050 has a
   2020 amendment.
3. **A paid standard whose figure we have not checked becomes a presence check,
   not a value check.** The gas rule asks whether ventilation exists; it never
   invents the free area the current edition requires.
4. **Statistics without methodology are opinion.** Tier E requires sample, field
   date and scope, and even then produces no findings.
5. **Manufacturers are a legitimate source on ergonomics, not on need.** Keep the
   motion study; drop the conclusion that the answer is their drawer.
6. **Foreign data is not local data.** American codes and European habits do not
   describe a Brazilian kitchen. Nobody labels them by hand: they declare the
   authority they come from, and the letter follows from where the project is.
7. **Every rule was born in a context.** The work triangle was calibrated for a
   one-person kitchen with no microwave and no dishwasher. Knowing when a rule
   was born tells you where it stops applying.

## Brazilian standards and law — A here, B abroad

| Code | Source | What it governs |
|---|---|---|
| `nbr15575` | ABNT NBR 15575-1:2021, performance standard | Minimum ceiling height and system performance |
| `nbr15575g` | ABNT NBR 15575-1:2021, **Annex F** (informative) | Minimum furniture and equipment, and the clearance around them |
| `nbr9050` | ABNT NBR 9050:2020 (amendment 1:2020) | Approach, reach ranges, control heights, wheelchair turning |
| `nbr13103` | ABNT NBR 13103:2024 (6th ed.) | Permanent ventilation where gas appliances are installed, up to 80 kW combined |
| `nbr5410` | ABNT NBR 5410 | One outlet per 3.5 m of perimeter; two above the countertop |
| `nbr8995` | ABNT NBR ISO/CIE 8995-1:2013 | Illuminance per activity, from 50 to 2,000 lx |
| `nbr16280` | ABNT NBR 16280 | Renovations in condominiums: plan, schedule and responsible engineer |
| `nbr14037` | ABNT NBR 14037 and NBR 5674 | Owner's manual and maintenance program |
| `nbr14810` | ABNT NBR 14810 | Particleboard (MDP): 551 to 750 kg/m³, good screw holding |
| `nbr15316` | ABNT NBR 15316 | MDF: dry-process fibers, machinable on face and edge |
| `caixa-mcmv` | Caixa, minimum specifications for housing units | 1.80 m kitchen, 120×50 sink, 55×60 stove, 70×70 fridge |
| `coe-municipal` | Municipal building code and state sanitary code | Areas, ventilation and the circle inscribed in the floor |
| `rdc216` | ANVISA RDC 216/2004 | Commercial kitchens: out of residential scope |

> **Municipal codes never become constants.** Their numbers are rows of the
> figure table under the authority of the city or the state that wrote them —
> São Paulo's decree 57.776 under `City { sao-paulo }`, the state sanitary code
> under `Region { sp }` — and the place the project declares (`Home.compass`, a
> city overridable for one call) decides whether they judge. A project in
> Curitiba is not measured against São Paulo's table, and one in Paraná does not
> hear São Paulo state's kitchen. `standards::MUNICIPAL_CODES` only names the
> places we hold a code for, for the picker.

## Foreign standards and codes — B here, A where they carry

| Code | Source | What it governs |
|---|---|---|
| `en1116` | EN 1116:2018 | Nominal widths of 400, 500, 600 and 900 mm, and built-in appliance niches |
| `nkba` | NKBA Kitchen & Bath Planning Guidelines (5th ed.) | Landing space beside cooktop and sink, walkways, wall cabinets |
| `irc2024` | IRC 2024 / NEC 2023, kitchen electrical (E3901, 210.52) | An outlet within 6 ft of any point of a wall, and within 24 in of any point of a counter |
| `irc2024-planning` | IRC 2024, chapter 3 — Building Planning | Room areas (R304) and sides, ceilings (R305), daylight (R303), fixture clearances (R307.1), hallways (R311.6) |
| `icc-a117` | ICC/ANSI A117.1-2009 | Turning space, door clear width, reach ranges, work surfaces, clear floor space at a basin |

What the United States pack does *not* hold yet — state adoption of the model
codes, the electrical and plumbing projects against the NEC and the IPC, egress
— is written at the top of `standards/us.rs` and in
[JURISDICTIONS.md](JURISDICTIONS.md).

> **Common mistake:** DIN 68935 covers bathroom furniture, not kitchens. For
> kitchens the reference is EN 1116.

## Figures: the numbers, and who demands them

A rule keeps its arithmetic — a work triangle is a formula, not a number — and
asks the figure table for what it compares against: `figure("clearance.bed.side",
place)`. A `Figure` is one number with the authority that demands it:

| Field | What it answers |
|---|---|
| `name` | What the rule asks for: `room.bedroom.min_area`, `clearance.toilet.front.wheelchair` |
| `authority` | Who demands it, and so where it applies — on the figure and not only on its source, because `coe-municipal` stands for every municipal code at once |
| `source` | The registry code a finding cites. `None` is common practice that no publication states: it cites nothing and nothing caps it, rather than inventing a source or demoting the finding |
| `bound` | `AtLeast`, `AtMost` — so "the stricter one wins" means something — or `Target`, a value a rule measures a deviation from |
| `severity` | What the finding weighs when the figure is missed, before the tier caps it |
| `note` | The article or table it comes from |

At a place, `figure()` returns the strictest row whose authority carries there,
and says whether the place *named* that authority (`Resolved::named`): São
Paulo's free circle accuses only a project that says it is in São Paulo, and a
kitchen whose city was left unsaid hears "which city?" instead. Where no row
carries at all, the home country's own national rows still answer, as the
reference they are there — a bedroom in Berlin is measured by annex F and the
finding cites annex F at letter B. A city's or a state's rows never travel, and
a foreign country's rows never speak at home.

The formula's parameters are figures too when somebody published them — the
63 % of stature the elbow sits at, 1,2 times the stature a shelf is reached at.
What stays in the rule is geometry and classification: which band of a face is
read, what counts as above the counter, that a double bed needs both sides.
None of that changes at a border.

The table is resolved once per review (`figures_at`), and three tests keep it
honest: every figure a rule asks for exists; every figure resolves at every
place unless the home pack's rows for it are a city's or a state's; and every
source a figure cites is in the registry.

## Design lineage — C wherever it is read

Every kitchen rule in circulation today was born at one of these points.

| Year | Code | Who, and what they brought |
|---|---|---|
| 1926 | — | **Frankfurt Kitchen**, Margarete Schütte-Lihotzky. 1.90 × 3.44 m, about 10,000 units. The origin of the fitted kitchen and of layout as a measurable problem. |
| 1929 | `gilbreth-triangulo` | **Lillian Moller Gilbreth** presents the work triangle; the Illinois Small Homes Council formalizes it in the lab in the 1940s. Calibrated for a one-person kitchen. |
| 1952 | — | **Charlotte Perriand and Le Corbusier**, Unité d'Habitation: the open bar kitchen, the birth of the integrated kitchen. |
| 1977 | `alexander184` | **Christopher Alexander**, pattern 184: total counter ≥ 366 cm besides sink, stove and fridge; no stretch < 122 cm; no pair > 305 cm apart. A free-standing table counts. |
| today | `blum-zonas` | **Blum and Hettich**: five zones in workflow order, and a counter 15–20 cm below the bent elbow. |
| today | `bulthaup-b1` | **bulthaup**: island, wall run and tall block. A useful typology; brand material without a published method. |
| — | `neufert` | **Neufert** (1936): the origin of the 90 × 60 cm counter that became the market standard. |
| — | `panero-zelnik` | **Panero & Zelnik**: reach and clearances by population percentile. |

## Laboratory research — D wherever it is read

| Code | Source | Finding |
|---|---|---|
| `lbnl-coifa` | Lawrence Berkeley National Laboratory | Across seven range hoods from US$ 40 to US$ 650, capture ranged from **15 % to 98 %**, and price did not predict performance. About 80 % on the back burners versus about 50 % on the front. Airflow alone does not tell how much pollution leaves the house. |
| `ibge-adensamento` | IBGE | Above three residents per bedroom a household is overcrowded. |

## Market data — E wherever it is read

Kept in the registry as context; by construction they **produce no findings**.

| Code | Source | Declared scope |
|---|---|---|
| `houzz2026` | 2026 U.S. Houzz Kitchen Trends Study | 1,780 American respondents, fieldwork in July 2025. Above-average income and engagement; nothing transfers to Brazil. |
| `abimovel` | Abimóvel, Brazilian furniture yearbook | 2024: 22 thousand companies, revenue above R$ 91.5 billion. |

## Where each source fits in the software

| Source | Surface | Status |
|---|---|---|
| `nbr5413` / `nbr8995` | `lighting` · `newera-core/src/lighting.rs` | each reference lux cites the table it comes from — 5413's residential values, 8995-1's for offices, laundries and dining, none for the outdoor figure that is only practice |
| `nbr9050` | `ergonomics` | turning circle, door width, outlet and wall-cabinet reach, `wheelchair` profile |
| `nbr15575` / `nbr15575g` | `ergonomics` | ceiling height, minimum furniture, clearance around pieces |
| `coe-municipal` | `ergonomics` + the project's place | inscribed circle, areas and windows |
| `caixa-mcmv` | `ergonomics` | minimum kitchen width |
| `nbr13103` | `ergonomics` | gas appliance without a permanent opening |
| `nbr5410` | `ergonomics` + `electrical` | outlets per perimeter and above the counter, where there is an electrical plan; the electrical project itself |
| `irc2024` / `irc2024-planning` / `icc-a117` | `ergonomics`, in the United States | the same rules, answering to the IRC and A117.1 where the project says it is there |
| `alexander184` | `ergonomics` | total counter, shortest stretch, distance between pairs |
| `blum-zonas` | `ergonomics` | the five zones, counter height by stature |
| `gilbreth-triangulo` | `ergonomics` | the triangle, with the limits of the context it was born in |
| `lbnl-coifa` | `ergonomics` + catalog (`hood`) | cooking without capture, hood narrower than the cooktop |
| `nkba` | `ergonomics` | 60 cm between sink and cooktop, wall-cabinet height above the counter |
| `en1116` | `newera-ergonomics/src/scene.rs` · `cabinet_run` | treats dishwasher, oven and microwave as niche appliances, not counter; `cabinet_run` defaults are the nominal widths |
| `nbr14810` / `nbr15316` | `cut_list` | panel definitions, in the response `refs` |
| `nbr16280` / `nbr14037` | — | process and handover: project metadata, not geometry |

### A note on EN 1116 and custom joinery

EN 1116 exists so that appliances, fronts and hardware from different makers are
interchangeable in a factory kitchen. `cabinet_run` produces custom joinery,
with a cut list and edge banding: there, four equal 61.25 cm doors in a 245 cm
run beat 60 + 60 + 60 + 65, and that is how the divider was built. The standard
applies where it actually governs, in how built-in appliances are classified and
in the module defaults; the rest is a design choice, not a deviation.

---

Compiled on September 15, 2026 from direct reading of the sources listed, and
revised on September 27, 2026 when the tier stopped being a field and became a
relation between a source and a place, the numbers left the rule bodies for the
figure table, and the United States pack joined.
Entries marked `ConfirmBeforeUse` in the registry were not confirmed against the
primary publication.
