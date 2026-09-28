# Ergonomics: how the review scores

- It never blocks anything: it reads the drawing and says what it finds, and the drawing
  stays the user's — somebody sketching to learn or to see an idea is not stopped by a
  standard.
- It covers room to walk beside beds and in front of kitchen equipment; beds, seats,
  bathrooms and wardrobes per person; the kitchen (work triangle, counter heights,
  Alexander's counter lengths, five work zones, sockets, gas ventilation, extraction);
  doors, ceiling heights, windows, minimum furniture and wheelchair turning.
- People (occupants, children, elderly, wheelchair, stature cm, city, scope) given to the
  call weigh that review only; `edit_home(people=…)` keeps them with the project, and every
  review and dry run then scores for them.
- `scope={electrical:false, plumbing:false}` scores architecture only; all findings remain
  visible, excluded ones weigh zero. Rooms and pieces are reviewed on the active storey, as
  `layout` is; `capacity` and the findings about the house (beds, bathrooms, seats,
  wardrobes per person) count every storey. `coverage` declares the limits.
- Scores are heuristic, not project completion or certification. `weight` is what the score
  would gain if that finding went away, so a score that moved can be read.
- `src` is the source a finding stands on, empty when it is common practice; resolve it in
  `sources` instead of asking.
- `tier` is the reliability ladder, read against where the project is: A obliges (a law,
  standard or utility spec that carries there) · B references (the same kind of source from
  somewhere else, or an association's guidance) · C doctrine, including a standard since
  withdrawn · D measured · E survey. It is why a finding is an error or only a tip. The same
  source is A or B depending on the place, so `edit_home(country=…, region=…, city=…)` is
  what makes the ladder right.
- `city`, e.g. `sao-paulo`, lets the municipal code judge instead of only advising; against
  a standard the more restrictive one wins. Set it once with `edit_home(city=…)` so dry runs
  and layout weigh the same rules.
- `fix`, when present, is a checked change as tool arguments (move or update): apply one,
  then review again (fixes of one review may overlap).
- `key` names the finding for `accept`: a finding accepted there stays in the report with
  its reason and stops costing score, which is what lets a correct plan reach zero
  pendencies honestly. `orphaned [[key, reason]]` lists acceptances no current finding
  answers to — the problem was fixed, and would come back already silenced;
  `accept(prune=true)` drops them.
