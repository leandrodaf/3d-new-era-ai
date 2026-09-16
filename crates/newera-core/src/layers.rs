//! Layers of the plan: lighting, appliances and joinery, shown or hidden on
//! the drawing while the 3D keeps everything.
//!
//! A piece is in a layer by what it is, so it is born there — a pendant is
//! lighting, a fridge an appliance, a cabinet built by the joinery tool
//! joinery — with no field to set and nothing to forget. `plan:layer` on a
//! piece overrides it (`lighting`, `appliances`, `joinery`, or `none`).

use serde::{Deserialize, Serialize};

use crate::furniture::Furniture;

/// A layer of the plan drawing.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PlanLayer {
    /// Lamps, spots, pendants, LED panels and strips.
    Lighting,
    /// Fridge, stove, oven, cooktop, hood, dishwasher, washer, dryer, microwave.
    Appliances,
    /// Cabinets, wardrobes, countertops, panels: what a joiner builds.
    Joinery,
}

impl PlanLayer {
    pub const ALL: [Self; 3] = [Self::Lighting, Self::Appliances, Self::Joinery];

    pub fn name(self) -> &'static str {
        match self {
            Self::Lighting => "Iluminação",
            Self::Appliances => "Eletrodomésticos",
            Self::Joinery => "Marcenaria",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Lighting => "lighting",
            Self::Appliances => "appliances",
            Self::Joinery => "joinery",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.key() == raw.trim())
    }
}

/// The property that puts a piece in a layer by hand.
pub const LAYER_KEY: &str = "plan:layer";

const LIGHTING_CATALOG: [&str; 6] = [
    "floor-lamp",
    "table-lamp",
    "downlight",
    "pendant",
    "led-panel",
    "led-strip",
];
const LIGHTING_WORDS: [&str; 12] = [
    "lumin",
    "lampad",
    "lamp",
    "abajur",
    "pendente",
    "spot",
    "plafon",
    "arandela",
    "fita led",
    "painel led",
    "led strip",
    "lustre",
];
const APPLIANCE_CATALOG: [&str; 9] = [
    "fridge",
    "stove",
    "hood",
    "dishwasher",
    "microwave",
    "cooktop",
    "oven",
    "washer",
    "dryer",
];
/// Words that make a piece joinery even when it names an appliance: the
/// separator above a cooktop's drawers, the stone over a dishwasher.
const JOINERY_STRONG: [&str; 16] = [
    "arremate",
    "roda-teto",
    "rodateto",
    "montante",
    "separador",
    "moldura",
    "gavet",
    "tampo",
    "bancada",
    "peninsula",
    "embutid",
    "planejad",
    "basculante",
    "nicho",
    "marcenaria",
    "armario",
];
const APPLIANCE_WORDS: [&str; 20] = [
    "tv ",
    "televis",
    "soundbar",
    "subwoofer",
    "geladeira",
    "refrigerador",
    "freezer",
    "fogao",
    "forno",
    "cooktop",
    "coifa",
    "depurador",
    "lava-louca",
    "lava louca",
    "maquina de lavar",
    "lava e seca",
    "secadora",
    "micro-ondas",
    "adega",
    "fridge",
];
const JOINERY_CATALOG: [&str; 7] = [
    "base-cabinet",
    "wall-cabinet",
    "tall-cabinet",
    "wardrobe",
    "kitchen-island",
    "basin-cabinet",
    "sink-counter",
];
const JOINERY_WORDS: [&str; 14] = [
    "armario",
    "gabinete",
    "guarda-roupa",
    "aereo",
    "bancada",
    "nicho",
    "painel ripado",
    "marcenaria",
    "prateleira",
    "torre",
    "vassoureiro",
    "balcao",
    "rack",
    "estante",
];

/// The layer a piece is in by itself, if any.
pub fn layer_of(piece: &Furniture) -> Option<PlanLayer> {
    if let Some(chosen) = piece.properties.get(LAYER_KEY) {
        return PlanLayer::parse(chosen);
    }
    if piece.is_opening() || piece.discipline.is_some() {
        return None;
    }
    let name = crate::annotations::fold(&piece.name);
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    let catalog = piece.catalog.as_str();
    if piece.light.is_some() || LIGHTING_CATALOG.contains(&catalog) || has(&LIGHTING_WORDS) {
        return Some(PlanLayer::Lighting);
    }
    if has(&JOINERY_STRONG) {
        return Some(PlanLayer::Joinery);
    }
    if APPLIANCE_CATALOG.contains(&catalog) || has(&APPLIANCE_WORDS) {
        return Some(PlanLayer::Appliances);
    }
    if piece.properties.contains_key("joinery:params")
        || piece.properties.contains_key("joinery:part")
        || JOINERY_CATALOG.contains(&catalog)
        || has(&JOINERY_WORDS)
    {
        return Some(PlanLayer::Joinery);
    }
    None
}

/// The layer a piece of a group is drawn in: its own when it has one — the
/// oven built into a tower is an appliance, the spot in a shelf lighting —
/// else its group's.
pub fn layer_in_group(top: &Furniture, part: &Furniture) -> Option<PlanLayer> {
    if part.id != top.id
        && let Some(own) = layer_of(part)
        && (part.properties.contains_key(LAYER_KEY) || own != PlanLayer::Joinery)
    {
        return Some(own);
    }
    layer_of(top)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::FurnitureId;

    fn piece(id: u64, catalog: &str, name: &str) -> Furniture {
        Furniture {
            id: FurnitureId(id),
            catalog: catalog.into(),
            name: name.into(),
            width: 60.0,
            depth: 60.0,
            height: 90.0,
            ..Furniture::default()
        }
    }

    #[test]
    fn choosing_architecture_shows_architecture_and_a_project_shows_itself() {
        use crate::style::Discipline;
        let mut doc = crate::Document::default();
        doc.choose_view(Some(Discipline::Electrical));
        assert_eq!(doc.home().active_discipline, Some(Discipline::Electrical));
        assert!(
            !doc.home()
                .hidden_disciplines
                .contains(&Discipline::Electrical)
        );

        doc.choose_view(None);
        assert_eq!(doc.home().active_discipline, None);
        assert!(
            doc.home()
                .hidden_disciplines
                .contains(&Discipline::Electrical)
        );
        assert!(
            doc.home()
                .hidden_disciplines
                .contains(&Discipline::Plumbing)
        );
        assert!(!doc.home().shown_in_3d(Some(Discipline::Electrical), None));

        doc.set_show_all_in_3d(true);
        assert!(
            doc.home()
                .shown_in_3d(Some(Discipline::Electrical), Some(PlanLayer::Joinery))
        );
    }

    #[test]
    fn pieces_are_born_in_their_layer() {
        assert_eq!(
            layer_of(&piece(1, "pendant", "Pendente")),
            Some(PlanLayer::Lighting)
        );
        assert_eq!(
            layer_of(&piece(2, "box", "Fita LED sob o aéreo")),
            Some(PlanLayer::Lighting)
        );
        assert_eq!(
            layer_of(&piece(3, "fridge", "Geladeira")),
            Some(PlanLayer::Appliances)
        );
        assert_eq!(
            layer_of(&piece(4, "imported", "Forno de embutir Brastemp")),
            Some(PlanLayer::Appliances)
        );
        assert_eq!(
            layer_of(&piece(5, "base-cabinet", "Armário")),
            Some(PlanLayer::Joinery)
        );
        assert_eq!(
            layer_of(&piece(6, "box", "Vassoureiro extraível")),
            Some(PlanLayer::Joinery)
        );
        assert_eq!(layer_of(&piece(7, "sofa-3", "Sofá")), None);
        // Names from a real plan.
        for joinery in [
            "Arremate em madeira junto ao teto",
            "Roda-teto do vassoureiro — face do corredor",
            "Mesa embutida — montante esquerdo fixo",
            "Cooktop — separador superior dos gavetões",
            "Península — pedra sobre lava-louças 60,5 cm",
            "Tampo contínuo sobre lava e seca até fachada",
            "Lavanderia — gavetões de roupas e cestos",
            "Mesa basculante FECHADA — oliva",
            "Armário 63,8 × 57 × 87 cm",
        ] {
            assert_eq!(
                layer_of(&piece(20, "imported", joinery)),
                Some(PlanLayer::Joinery),
                "{joinery}"
            );
        }
        for appliance in [
            "LG WD18GNTS6BA — Lava e Seca 18 kg",
            "Micro-ondas Brastemp BMG45AE",
            "Cooktop Brastemp BDS62AE — 4 bocas",
            "TV sala — fixada na parede sem pés",
        ] {
            assert_eq!(
                layer_of(&piece(21, "imported", appliance)),
                Some(PlanLayer::Appliances),
                "{appliance}"
            );
        }
        for neither in [
            "Sofá Milano",
            "Cadeira Dover",
            "Box social — folhas de correr",
            "Persiana integrada",
        ] {
            assert_eq!(layer_of(&piece(22, "imported", neither)), None, "{neither}");
        }

        // Chosen by hand, it wins either way.
        let mut sofa = piece(8, "sofa-3", "Sofá");
        sofa.properties.insert(LAYER_KEY.into(), "joinery".into());
        assert_eq!(layer_of(&sofa), Some(PlanLayer::Joinery));
        let mut fridge = piece(9, "fridge", "Geladeira");
        fridge.properties.insert(LAYER_KEY.into(), "none".into());
        assert_eq!(layer_of(&fridge), None);

        // In a tower of joinery, the oven is an appliance and a board is joinery.
        let mut tower = piece(10, "box", "Torre quente");
        tower.children = vec![piece(11, "oven", "Forno"), piece(12, "box", "lateral")];
        assert_eq!(
            layer_in_group(&tower, &tower.children[0]),
            Some(PlanLayer::Appliances)
        );
        assert_eq!(
            layer_in_group(&tower, &tower.children[1]),
            Some(PlanLayer::Joinery)
        );
    }
}
