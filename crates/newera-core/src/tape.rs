//! Adhesive flat wiring tape (Eletrofitas, the one brand sold in Brazil):
//! what each model carries, what a run of it takes to buy, and where it may
//! not go.
//!
//! The tape is copper tracks between polycarbonate faces, glued to the
//! surface of a wall or ceiling, covered by a glass-fibre mesh and filler and
//! painted over. It is not a conductor NBR 5410 covers and has no Inmetro
//! certification, so a run of it is offered as an option with that said, and
//! refused where it cannot be right: a socket needs protective earth
//! (NBR 5410 6.5.3.1), which only the 3-track EF18.9.18 carries; a current
//! over its rating; wet rooms, where the maker gives no rating.
//! Figures from eletrofitas.com.br and Leroy Merlin's product pages.

use serde::Serialize;

/// What a model is sold for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TapeUse {
    /// Lighting and loads with no earth.
    Lighting,
    /// Two-pin sockets, no earth.
    TwoPin,
    /// Three-pin sockets and air conditioning: phase, earth, neutral.
    Earthed,
    /// Two-way switching runs.
    Parallel,
    /// Ceiling fan with its light.
    Fan,
    /// Sound, alarm, phone, audio: signal, not power.
    Signal,
}

/// One tape model with the kits Leroy Merlin sells.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TapeModel {
    pub model: &'static str,
    pub tracks: u8,
    pub amps: f64,
    /// Round wire it is equivalent to, mm².
    pub equivalent_mm2: f64,
    pub width_mm: f64,
    pub usage: TapeUse,
    /// Connectors a kit brings: one per track at each end.
    pub connectors: u8,
    /// Kits: (length m, Leroy code, price R$).
    pub kits: &'static [(f64, &'static str, f64)],
}

impl TapeModel {
    pub fn earthed(self) -> bool {
        self.usage == TapeUse::Earthed
    }
}

/// The models, lightest first.
pub static MODELS: &[TapeModel] = &[
    TapeModel {
        model: "EF5x2",
        tracks: 2,
        amps: 10.0,
        equivalent_mm2: 1.0,
        width_mm: 30.0,
        usage: TapeUse::Signal,
        connectors: 4,
        kits: &[
            (2.0, "91920871", 88.90),
            (3.0, "91920885", 114.90),
            (5.0, "91920892", 179.90),
        ],
    },
    TapeModel {
        model: "EF9x2",
        tracks: 2,
        amps: 15.0,
        equivalent_mm2: 1.5,
        width_mm: 60.0,
        usage: TapeUse::Lighting,
        connectors: 4,
        kits: &[
            (1.5, "91920724", 104.90),
            (2.0, "91920731", 119.90),
            (5.0, "91920745", 209.90),
        ],
    },
    TapeModel {
        model: "EF9x3",
        tracks: 3,
        amps: 15.0,
        equivalent_mm2: 1.5,
        width_mm: 60.0,
        usage: TapeUse::Parallel,
        connectors: 6,
        kits: &[
            (1.5, "91920822", 97.90),
            (2.0, "91920836", 114.90),
            (3.0, "91920843", 144.90),
            (5.0, "91920850", 214.90),
        ],
    },
    TapeModel {
        model: "EF18x2",
        tracks: 2,
        amps: 20.0,
        equivalent_mm2: 2.5,
        width_mm: 60.0,
        usage: TapeUse::TwoPin,
        connectors: 4,
        kits: &[
            (1.5, "91920752", 119.90),
            (2.0, "91920766", 139.90),
            (3.0, "91920773", 174.90),
            (5.0, "91920780", 249.90),
        ],
    },
    TapeModel {
        model: "EF18.9.18",
        tracks: 3,
        amps: 20.0,
        equivalent_mm2: 2.5,
        width_mm: 60.0,
        usage: TapeUse::Earthed,
        connectors: 6,
        kits: &[(3.0, "91920801", 179.90), (5.0, "91920815", 209.90)],
    },
    TapeModel {
        model: "EF5x5",
        tracks: 5,
        amps: 10.0,
        equivalent_mm2: 1.0,
        width_mm: 60.0,
        usage: TapeUse::Fan,
        connectors: 4,
        kits: &[
            (1.5, "91920941", 104.90),
            (2.0, "91920955", 119.90),
            (3.0, "91920962", 159.90),
            (5.0, "91920976", 224.90),
        ],
    },
    TapeModel {
        model: "EF5x4",
        tracks: 4,
        amps: 10.0,
        equivalent_mm2: 1.0,
        width_mm: 60.0,
        usage: TapeUse::Signal,
        connectors: 8,
        kits: &[
            (5.0, "91920934", 209.90),
            (8.0, "91920906", 289.90),
            (10.0, "91920913", 344.90),
            (12.0, "91920920", 404.90),
        ],
    },
];

/// Extra connector kits: (model it fits, Leroy code, name, price R$).
pub static CONNECTOR_KITS: &[(&str, &str, &str, f64)] = &[
    (
        "EF9x2",
        "88022662",
        "Kit de conectores 1 pino 10 A azul EF9x2 (iluminação)",
        22.90,
    ),
    (
        "EF18x2",
        "88022984",
        "Kit de conectores 2 pinos 20 A laranja EF18x2",
        33.90,
    ),
    (
        "EF18.9.18",
        "88022956",
        "Kit de conectores 3 pinos 20 A verde EF18.9.18",
        38.90,
    ),
];

/// Mesh sold apart: (length m, Leroy code, price R$).
pub static MESH: &[(f64, &str, f64)] = &[(5.0, "89000702", 19.99), (15.0, "89000716", 62.90)];

/// Every connection takes 20 cm of tape (maker's installation guide).
pub const PER_CONNECTION_CM: f64 = 20.0;

/// The model for a power run: an earthed one when any point is a socket or a
/// dedicated load (NBR 5410 6.5.3.1), the lightest that carries the current
/// otherwise. Why not, when none can.
pub fn model_for(amps: f64, needs_earth: bool) -> Result<&'static TapeModel, String> {
    let candidates: Vec<&TapeModel> = MODELS
        .iter()
        .filter(|m| {
            if needs_earth {
                m.earthed()
            } else {
                matches!(
                    m.usage,
                    TapeUse::Lighting | TapeUse::TwoPin | TapeUse::Earthed
                )
            }
        })
        .collect();
    candidates
        .iter()
        .copied()
        .find(|m| m.amps + 1e-9 >= amps)
        .ok_or_else(|| {
            let best = candidates.iter().map(|m| m.amps).fold(0.0, f64::max);
            format!(
                "{} A passa do que a fita aguenta ({best} A{}): leve este circuito em fio, dentro de eletroduto",
                crate::electrical::decimal(amps),
                if needs_earth { " com terra, na EF18.9.18" } else { "" }
            )
        })
}

/// A line to buy.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Purchase {
    pub item: String,
    pub code: &'static str,
    pub quantity: u32,
    pub unit_price: f64,
}

/// The kits for pieces of tape of the given lengths, cm (each piece already
/// holding its 20 cm per end): the cheapest kit that covers each piece; a
/// piece longer than the longest kit takes several, spliced in a box with a
/// connector kit per splice.
///
/// # Panics
///
/// Never for the models of [`MODELS`], which all have kits.
pub fn purchase(model: &TapeModel, pieces_cm: &[f64]) -> (Vec<Purchase>, u32) {
    let mut count: std::collections::BTreeMap<&str, (f64, f64, u32)> =
        std::collections::BTreeMap::new();
    let mut splices = 0;
    let longest = model.kits.iter().map(|k| k.0).fold(0.0, f64::max);
    for cm in pieces_cm {
        let mut left = cm / 100.0;
        loop {
            let kit = model
                .kits
                .iter()
                .filter(|k| k.0 + 1e-9 >= left)
                .min_by(|a, b| a.2.total_cmp(&b.2))
                .copied()
                .unwrap_or_else(|| {
                    *model
                        .kits
                        .iter()
                        .find(|k| (k.0 - longest).abs() < 1e-9)
                        .expect("kits")
                });
            let entry = count.entry(kit.1).or_insert((kit.0, kit.2, 0));
            entry.2 += 1;
            if kit.0 + 1e-9 >= left {
                break;
            }
            left -= kit.0 - 2.0 * PER_CONNECTION_CM / 100.0;
            splices += 1;
        }
    }
    let mut out: Vec<Purchase> = count
        .into_iter()
        .map(|(code, (metres, price, n))| Purchase {
            item: format!(
                "Kit fita elétrica adesiva {} {} pistas {} A 750 V, {} m (fita, {} conectores e malha)",
                model.model,
                model.tracks,
                model.amps,
                crate::electrical::decimal(metres).trim_end_matches(",0"),
                model.connectors
            ),
            code,
            quantity: n,
            unit_price: price,
        })
        .collect();
    if splices > 0
        && let Some((_, code, name, price)) = CONNECTOR_KITS.iter().find(|c| c.0 == model.model)
    {
        out.push(Purchase {
            item: format!("{name} (emendas)"),
            code,
            quantity: splices * 2,
            unit_price: *price,
        });
    }
    (out, splices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_socket_run_takes_the_earthed_tape_and_nothing_over_its_rating() {
        assert_eq!(model_for(8.0, true).unwrap().model, "EF18.9.18");
        assert_eq!(model_for(8.0, false).unwrap().model, "EF9x2");
        assert_eq!(model_for(17.0, false).unwrap().model, "EF18x2");
        let err = model_for(25.0, true).unwrap_err();
        assert!(err.contains("fio"), "{err}");
        assert!(MODELS.iter().filter(|m| m.earthed()).all(|m| m.tracks == 3));
    }

    #[test]
    fn kits_cover_each_piece_cheapest_and_long_pieces_are_spliced() {
        let model = model_for(8.0, true).unwrap();
        // 2,4 m fits the 3 m kit; 4,2 m the 5 m one.
        let (buy, splices) = purchase(model, &[240.0, 420.0]);
        assert_eq!(splices, 0);
        assert!(
            buy.iter().any(|p| p.code == "91920801" && p.quantity == 1),
            "{buy:?}"
        );
        assert!(
            buy.iter().any(|p| p.code == "91920815" && p.quantity == 1),
            "{buy:?}"
        );
        // 7 m is past the 5 m kit: a splice and its connectors.
        let (buy, splices) = purchase(model, &[700.0]);
        assert_eq!(splices, 1, "{buy:?}");
        assert!(
            buy.iter().any(|p| p.code == "88022956" && p.quantity == 2),
            "{buy:?}"
        );
    }
}
