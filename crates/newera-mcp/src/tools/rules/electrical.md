# Electrical and telecom: what the review checks

NBR 5410 (low-voltage installations) and NBR 16264 (residential cabling). Points are the
electrical pieces of the catalog (outlets, switches, lighting points, panel, network-outlet
RJ45, tv-outlet, wifi-point, telecom-panel) plus every fixture that lights.

## check

- A ceiling lighting point per room.
- General-use outlets per room: kitchens and laundries one per 3.5 m of perimeter,
  bathrooms one by the basin, living rooms and bedrooms one per 5 m.
- RJ45 and TV outlets per room as NBR 16264 table 1 recommends: 2 RJ45 and 1 TV in
  bedrooms, living rooms, offices, kitchens and laundries; 3 and 2 in a home theater;
  1 and 1 elsewhere. A Wi-Fi point is not an RJ45 outlet.
- A power outlet by the telecom panel; a distribution and a telecom panel.
- Points without a circuit; a dedicated load not alone.
- Lighting and outlets sharing a circuit only when NBR 5410 9.5.3.3 forbids it (over
  16 A, or all lighting or all outlets on shared circuits); kitchen and laundry outlets
  sharing with anything else; equipment over 10 A not alone.
- A voltage drop over 4 % to the farthest point of a laid-out run.
- The panel: when it does not fit, when the main breaker is under twice the largest
  partial (selectivity), and when the short-circuit level was assumed.
- Each access point: its data cable, power (an outlet within 1.5 m, or PoE) and a cable
  category that carries its uplink (up to 2.5 GbE: Cat 5e; 5 GbE: Cat 6; 10 GbE: Cat 6A).
- Automation: a relay or smart switch needs a neutral in its box; a dimmer needs the
  room's lighting to fit its max_w (default 1.1 A at the supply voltage, some 140 W at
  127 V) and pass 10 W; a ceiling presence sensor between 2.2 and 3 m (manufacturers
  install at about 2.4) that sees the room's far corner (about 1.45 × its height); a
  smart lock on a door.

## circuits

- Rows `[name, kinds, points, VA, V, A, wire mm², breaker A, DR]`.
- Power by NBR 5410 unless written: lighting per room by area, 100 VA up to 6 m² and 60 VA
  per whole 4 m² beyond, shared by its points; 600 VA for each of the first three outlets
  of a kitchen, laundry or bathroom, 100 VA after and elsewhere; dedicated points at their
  rated power, a shower 7500 W and air conditioning 1500 until written.
- The wire section is the smallest from 1.5 mm² (lighting) or 2.5 (with outlets) whose
  capacity, corrected for the circuits sharing its conduit on the laid-out runs (table 42,
  or grouping written on the project), admits a breaker between the current and it.
- DR on every point of a room with a bath or shower, on kitchen, laundry, service and
  garage points (lighting at 2.50 m or higher excepted) and on outdoor and balcony outlets.
  A shower over 4.4 kVA runs on 220 V.
- `main_breaker {a, phases, load_a_per_phase}`: the supply to ask for, by Enel SP's
  categories on 127/220 V (single-phase up to 12 kW with no 220 V circuit, two-phase up to
  20 kW, three-phase up to 75 kW) and the smallest of its fixed entry breakers (50, 63, 80,
  100… A).
- `panel {devices: [[device, count, DIN modules]], modules {used, capacity,
  capacity_written, spare}, dps, earthing, icn_ka, selective}`: one-pole breakers for 127 V
  circuits and two-pole for 220 V between phases, a two-pole DR per circuit that needs one,
  the main breaker, the surge protector (DPS class II, a module per phase and neutral) and
  NBR 5410's spare ways (2 up to 6 circuits, 3 up to 12, 4 up to 30, 15 % above). Capacity
  is the panel's modules (`edit_electrical` assign modules on it) or a guess from its size.
- Automation (catalog smart-relay, smart-switch, dimmer, presence-sensor, smart-lock) draws
  its standby on its circuit: relay, dimmer and sensor 1 W, smart switch 1.2 by default,
  from manufacturers' sheets (assign standby_w to change it); circuits reports standby_w.
- Circuit numbers are drawn next to the points on the plan, and with
  `edit_annotations(legend=true)` the load schedule under the legend.

## wifi

- `{band?: 2.4|5|6}` answers `access_points [[id, standard, bands, uplink]]`,
  `coverage [[room, band, median dBm, worst dBm (9 places in 10), share at -67 dBm or
  better, grade]]` and `suggested {standard, band, points: [[x, y, z, room]], short: [rooms
  still under -67 dBm]}`.
- Signal is estimated from free-space loss, distance and each wall crossed by its material
  and thickness (a door or window where the path goes through one), per band.
- The suggestion is the fewest ceiling points (up to four) at room centres covering the
  rooms people use.

## Runs and flat wiring tape (edit_electrical)

- `cable` draws a run told apart on the plan (power solid, network dashed, TV dash-dot);
  check then reports cables_m, the length by kind with a tenth for the drops, and network
  or TV points no run reaches, or a telecom panel none reaches.
- `route` lays the run the way it is built: along the walls and inside them (or in the
  slab), from the nearest panel of its kind to the points, sharing the trunk; it replaces
  the earlier run of the same circuit and the cables drawn by hand to its points (listed in
  replaced_drawn). materials lists conduit, boxes, wire by conductor, cable, connectors.
  A via that cannot reach a point (wall: a point out of every wall; ceiling: a low point out
  of every wall, nowhere to drop) is refused naming the points.
- `via=tape` lays adhesive flat wiring tape (Eletrofitas) on the surface of walls and
  ceiling, for power only: the model by the load and whether any point is a socket (sockets
  take the earthed EF18.9.18); refused over its rating, in a bathroom or outdoors; bought as
  Leroy Merlin kits by piece (codes, prices, splices), answered in
  `tape {model, tracks, rated_a, load_a, earthed, pieces_m, splices, total_brl, notes}`.
