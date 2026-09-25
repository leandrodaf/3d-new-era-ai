# Standards and sources

Where the numbers come from when the software says a kitchen is wrong.

This is the project's bibliography: the standards, design doctrine and research
behind each rule, with edition and year. It is not decorative. The registry
lives in code, in [`crates/newera-core/src/standards.rs`](../crates/newera-core/src/standards.rs),
and every `ergonomics` finding carries the short code of the source it stands
on. Each row below matches an entry there.

## The reliability ladder

Sources do not weigh the same, and treating them as equal is how you end up
with measurements nobody owns. The tier decides how strongly a finding may
accuse. It is `Tier` in the code, and the severity cap is enforced in
`push_ref`, not left to whoever writes the rule.

| Tier | What it is | Severity cap |
|---|---|---|
| **A · binding** | Brazilian standard or municipal code. Breaking it is a legal or safety problem. | `Erro` (error) |
| **B · reference** | Foreign standard or association guideline. Good engineering, no legal force here. | `Alerta` (warning) |
| **C · doctrine** | Architects and manufacturers. What makes a kitchen good, not what makes it legal. | `Dica` (tip) |
| **D · measured** | Laboratory research. Empirical data with a published method. | `Alerta` (warning) |
| **E · descriptive** | Market surveys. What people do, never what they should do. | no findings |

Besides the tier, every entry carries a `Confidence`. `ConfirmBeforeUse` marks
what was not checked against the primary publication: it may advise, never
accuse. That is why a 170 cm kitchen comes out as a warning against the Caixa
specification, not as an error, even though the rule asks for one.

## Honesty rules that count as code rules

1. **Every measurement has an owner.** A number without a source does not go
   into a constant.
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
   describe a Brazilian kitchen; they enter as tier B, labeled with their origin.
7. **Every rule was born in a context.** The work triangle was calibrated for a
   one-person kitchen with no microwave and no dishwasher. Knowing when a rule
   was born tells you where it stops applying.

## A · Brazilian standards

| Code | Source | What it governs |
|---|---|---|
| `nbr15575` | ABNT NBR 15575-1:2021, performance standard | Minimum ceiling height and system performance |
| `nbr15575g` | ABNT NBR 15575-1:2021, **Annex G** (informative) | Minimum furniture and equipment, and the clearance around them |
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

> **Municipal codes never become constants.** They live in
> `standards::MUNICIPAL_CODES`, keyed by city, and `Profile.city` decides whether
> they judge or only advise. Between a standard and local law, the stricter one
> wins. Today the registry has the city of São Paulo and the São Paulo state
> sanitary code; an unknown city becomes advice to confirm.

## B · Foreign standards and codes

| Code | Source | What it governs |
|---|---|---|
| `en1116` | EN 1116:2018 | Nominal widths of 400, 500, 600 and 900 mm, and built-in appliance niches |
| `nkba` | NKBA Kitchen & Bath Planning Guidelines (5th ed.) | Landing space beside cooktop and sink, walkways, wall cabinets |
| `irc2024` | IRC 2024 / NEC 2023 | Countertops from 305 mm need an outlet (a counterpoint to NBR 5410) |

> **Common mistake:** DIN 68935 covers bathroom furniture, not kitchens. For
> kitchens the reference is EN 1116.

## C · Design lineage

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

## D · Laboratory research

| Code | Source | Finding |
|---|---|---|
| `lbnl-coifa` | Lawrence Berkeley National Laboratory | Across seven range hoods from US$ 40 to US$ 650, capture ranged from **15 % to 98 %**, and price did not predict performance. About 80 % on the back burners versus about 50 % on the front. Airflow alone does not tell how much pollution leaves the house. |
| `ibge-adensamento` | IBGE | Above three residents per bedroom a household is overcrowded. |

## E · Market data

Kept in the registry as context; by construction they **produce no findings**.

| Code | Source | Declared scope |
|---|---|---|
| `houzz2026` | 2026 U.S. Houzz Kitchen Trends Study | 1,780 American respondents, fieldwork in July 2025. Above-average income and engagement; nothing transfers to Brazil. |
| `abimovel` | Abimóvel, Brazilian furniture yearbook | 2024: 22 thousand companies, revenue above R$ 91.5 billion. |

## Where each source fits in the software

| Source | Surface | Status |
|---|---|---|
| `nbr8995` | `lighting` · `newera-core/src/lighting.rs` | integrated first; `recommended_lux` is the model the rest followed |
| `nbr9050` | `ergonomics` | turning circle, door width, outlet and wall-cabinet reach, `wheelchair` profile |
| `nbr15575` / `nbr15575g` | `ergonomics` | ceiling height, minimum furniture, clearance around pieces |
| `coe-municipal` | `ergonomics` + `Profile.city` | inscribed circle, areas and windows |
| `caixa-mcmv` | `ergonomics` | minimum kitchen width |
| `nbr13103` | `ergonomics` | gas appliance without a permanent opening |
| `nbr5410` | `ergonomics` | outlets per perimeter and above the counter, where there is an electrical plan |
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

Compiled on September 15, 2026 from direct reading of the sources listed.
Entries marked `ConfirmBeforeUse` in the registry were not confirmed against the
primary publication.
