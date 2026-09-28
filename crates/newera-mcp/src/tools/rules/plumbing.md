# Plumbing: what the review checks

NBR 5626 (cold and hot water) and NBR 8160 (sewer).

## Points and fixtures

- Points are the plumbing pieces of the catalog: cold-water, hot-water, sewer, valve,
  grease-trap, inspection-box, water-meter, gas-point, vent-pipe, and the drains as the
  models they are:
  - floor-drain: caixa sifonada 150x150x50;
  - floor-drain-100; floor-drain-75 (150x185x75, up to 15 UHC);
  - trap-drain-small (seal under 50 mm, no trap); dry-drain;
  - linear-drain (w 50/70/90, no trap); linear-drain-trap;
  - rain-drain for open areas.
- Drains are set flush in the floor of a room, never in a wall, a door span or under a
  cabinet. What a point is comes from its catalog, never its name.
- Fixtures are the pieces that use water: toilet, basin, kitchen sink, shower, bathtub,
  washer, laundry sink, dishwasher.

## check

- A cold-water point by every fixture, and a sewer point (or a floor drain for basin,
  shower, tub and machines) within reach.
- The discharge diameter each fixture needs: toilet 100 mm, kitchen sink and machines 50,
  others 40.
- Hot water where the project has any.
- A floor drain in every bathroom, kitchen and laundry, inside the shower area where there
  is one; at least one real trap (50 mm seal) per room; the UHC its outlet takes (50 mm: 6,
  75 mm: 15); rain drains for open terraces.
- A grease trap for a kitchen sink.
- The premises: where the water comes from (water-meter or valve) and where the sewer goes
  (inspection-box or stack).
- Points no drawn pipe of their kind reaches.

## Runs (edit_plumbing)

- Water runs: pipe and bars, 90° elbows, tees, threaded elbows at the points, a gate valve
  per room, adhesive.
- Sewer: the branch at the largest diameter it takes, the drops at each point's own, a 45°
  Y junction at each branch (never a 90° tee), two 45° elbows per turn, sealing rings, trap
  boxes. It runs only under the floor, by gravity, with 2 % fall up to 75 mm and 1 %
  above; the reply says needs_depth_cm, and with depth a run that does not fit is refused
  saying how much it needs.
- Refused, with why: a point in no wall; from the ceiling, a low point in no wall with
  nowhere to drop; sewer up to the ceiling or lying in a wall.
