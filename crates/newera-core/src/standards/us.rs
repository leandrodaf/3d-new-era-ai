//! The United States: the codes that oblige a home there, and the numbers
//! they demand.
//!
//! A pack is data — who publishes what, and the figures under their authority
//! — and nothing in it moves a finding at home: every row here is a
//! `Country("us")` authority, and a foreign row never speaks in Brazil (see
//! [`super::figure`]). What made it more than data was done before it could be
//! written: a finding's discipline comes from its rule, not from the code it
//! cites, so a socket count standing on the NEC is still electrical; a room is
//! recognised in English; and a finding can be said in English.
//!
//! What this pack does not yet hold, on purpose:
//!
//! - **State adoption.** The IRC and the NEC are model codes; a state or a
//!   city adopts an edition, sometimes amended (Florida has its own Building
//!   Code, Residential; Texas leaves it to its cities). They stand here at the
//!   country's level, which is right for the numbers below — unchanged across
//!   those adoptions — and not a claim that every town enforces them.
//! - **The installation projects.** NBR 5410 against the NEC and NBR 8160
//!   against the IPC is a design problem — circuits by kind, VA per room, 120 V
//!   — not a translation, and the electrical and plumbing checks still speak
//!   the Brazilian standards. Abroad those findings arrive as references (see
//!   [`crate::electrical::weighed`]), which is honest until then.
//! - **Egress.** IRC R310 (a sleeping room's emergency escape opening) and
//!   R311.2 (the egress door) have no rule to answer to yet.
//!
//! - **A117.1's 2017 edition.** It widened the turning space and the clear
//!   floor space for new buildings (67 in, 30 × 52 in). The 2009 edition, the
//!   one the IBC 2015 and 2018 still reference, is what is written here.
//!
//! Every number is converted from the imperial figure the code states, and
//! the note says which.

use super::{Authority, Bound, Confidence, Figure, Kind, Severity, Standard, fig, s};

use Bound::{AtLeast, AtMost};
use Confidence::Verified;
use Severity::{Alerta, Dica, Erro};

const US: Authority = Authority::Country("us");

/// Centimetres in an inch.
const IN: f64 = 2.54;

/// Square centimetres in a square foot.
const SQ_FT: f64 = 929.0304;

/// Sources whose word carries in the United States.
pub(super) static STANDARDS: &[Standard] = &[
    s(
        "irc2024-planning",
        "IRC 2024 — International Residential Code, cap. 3: Building Planning",
        "2024",
        US,
        Kind::Law,
        "Área e dimensão mínimas de cômodos habitáveis (R304), pé-direito (R305), luz natural (R303), aparelhos sanitários (R307) e corredores (R311.6).",
        Some("https://codes.iccsafe.org/content/IRC2024P1/chapter-3-building-planning"),
        Verified,
    ),
    s(
        "icc-a117",
        "ICC/ANSI A117.1-2009 — Accessible and Usable Buildings and Facilities",
        "2009",
        US,
        Kind::Standard,
        "Espaço de giro, portas, alcances e áreas de aproximação: a norma que a unidade acessível segue.",
        Some("https://codes.iccsafe.org/content/ICCA117_12009"),
        Verified,
    ),
];

// Six fields, every row: `fig` with the authority already said.
const fn us(
    name: &'static str,
    source: &'static str,
    bound: Bound,
    severity: Severity,
    note: &'static str,
) -> Figure {
    fig(name, US, Some(source), bound, severity, note)
}

/// What the IRC, the NEC and A117.1 demand, under the names the rules ask for.
pub(super) static FIGURES: &[Figure] = &[
    // ---- The room itself ----
    us(
        "room.bedroom.min_area",
        "irc2024-planning",
        AtLeast(70.0 * SQ_FT),
        Dica,
        "IRC R304.1: 70 sq ft",
    ),
    us(
        "room.living.min_area",
        "irc2024-planning",
        AtLeast(70.0 * SQ_FT),
        Dica,
        "IRC R304.1: 70 sq ft",
    ),
    us(
        "room.bedroom.min_side",
        "irc2024-planning",
        AtLeast(84.0 * IN),
        Dica,
        "IRC R304.2: 7 ft",
    ),
    us(
        "room.living.min_side",
        "irc2024-planning",
        AtLeast(84.0 * IN),
        Dica,
        "IRC R304.2: 7 ft",
    ),
    us(
        "room.corridor.min_side",
        "irc2024-planning",
        AtLeast(36.0 * IN),
        Alerta,
        "IRC R311.6: 3 ft",
    ),
    us(
        "room.corridor.min_side.wheelchair",
        "icc-a117",
        AtLeast(36.0 * IN),
        Alerta,
        "A117.1 403.5.1: 36 in",
    ),
    us(
        "room.corridor.min_side.wheelchair.long",
        "icc-a117",
        AtLeast(36.0 * IN),
        Alerta,
        "A117.1 403.5.1: 36 in",
    ),
    // Habitable rooms and hallways 7 ft; bathrooms, toilet rooms and
    // laundries 6 ft 8 in.
    us(
        "room.min_ceiling",
        "irc2024-planning",
        AtLeast(84.0 * IN),
        Alerta,
        "IRC R305.1: 7 ft",
    ),
    us(
        "room.corridor.min_ceiling",
        "irc2024-planning",
        AtLeast(84.0 * IN),
        Alerta,
        "IRC R305.1: 7 ft",
    ),
    us(
        "room.bathroom.min_ceiling",
        "irc2024-planning",
        AtLeast(80.0 * IN),
        Alerta,
        "IRC R305.1: 6 ft 8 in",
    ),
    us(
        "room.laundry.min_ceiling",
        "irc2024-planning",
        AtLeast(80.0 * IN),
        Alerta,
        "IRC R305.1: 6 ft 8 in",
    ),
    // Glazing of at least 8 % of the floor of a habitable room.
    us(
        "room.floor_per_glass",
        "irc2024-planning",
        AtMost(12.5),
        Dica,
        "IRC R303.1",
    ),
    us(
        "room.floor_per_glass.work",
        "irc2024-planning",
        AtMost(12.5),
        Dica,
        "IRC R303.1",
    ),
    us(
        "room.wheelchair_turn",
        "icc-a117",
        AtLeast(60.0 * IN),
        Alerta,
        "A117.1 304.3.1: 60 in",
    ),
    // ---- Doors and reach ----
    us(
        "door.clear_width.wheelchair",
        "icc-a117",
        AtLeast(32.0 * IN),
        Alerta,
        "A117.1 404.2.3: 32 in",
    ),
    us(
        "reach.top.wheelchair",
        "icc-a117",
        AtMost(34.0 * IN),
        Alerta,
        "A117.1 902.3: 34 in",
    ),
    us(
        "reach.switch.min",
        "icc-a117",
        AtLeast(15.0 * IN),
        Alerta,
        "A117.1 308: 15 to 48 in",
    ),
    us(
        "reach.switch.max",
        "icc-a117",
        AtMost(48.0 * IN),
        Alerta,
        "A117.1 308: 15 to 48 in",
    ),
    us(
        "reach.outlet.min",
        "icc-a117",
        AtLeast(15.0 * IN),
        Alerta,
        "A117.1 308: 15 to 48 in",
    ),
    us(
        "reach.outlet.max",
        "icc-a117",
        AtMost(48.0 * IN),
        Alerta,
        "A117.1 308: 15 to 48 in",
    ),
    // ---- In front of fixtures, IRC figure R307.1 ----
    us(
        "clearance.toilet.front",
        "irc2024-planning",
        AtLeast(21.0 * IN),
        Alerta,
        "IRC R307.1: 21 in in front of a water closet",
    ),
    us(
        "clearance.basin.front",
        "irc2024-planning",
        AtLeast(21.0 * IN),
        Alerta,
        "IRC R307.1: 21 in in front of a lavatory",
    ),
    us(
        "clearance.shower.front",
        "irc2024-planning",
        AtLeast(24.0 * IN),
        Alerta,
        "IRC R307.1: 24 in in front of a shower opening",
    ),
    us(
        "clearance.basin.front.wheelchair",
        "icc-a117",
        AtLeast(48.0 * IN),
        Alerta,
        "A117.1 305.3: 30 × 48 in clear floor space",
    ),
    // ---- The kitchen's outlets, IRC E3901 / NEC 210.52 ----
    // No point along a wall more than 6 ft from an outlet: one every 12 ft.
    us(
        "kitchen.sockets.perimeter_each",
        "irc2024",
        AtMost(144.0 * IN),
        Erro,
        "IRC E3901.2 / NEC 210.52(A)(1): 6 ft to an outlet",
    ),
    us(
        "kitchen.sockets.over_counter",
        "irc2024",
        AtLeast(2.0),
        Erro,
        "IRC E3901.4 / NEC 210.52(C): no point of a counter more than 24 in from an outlet",
    ),
];
