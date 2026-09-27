//! What a room's name says it is, in every language we hold.
//!
//! Five checks used to read a room by grepping its name for Portuguese words,
//! each with a list of its own: the review, the electrical and plumbing
//! projects, the lighting and the Wi-Fi. A project whose rooms are called
//! "Bedroom" and "Kitchen" tripped none of them — not because nothing applied,
//! but because nothing recognised the room — and the lists disagreed with each
//! other about Portuguese too.
//!
//! So the words live here, once, one table per language, and each word says
//! what it *means*: a [`Mention`]. A name can mention more than one thing — a
//! "Sala e cozinha" is a living room to one check and a kitchen to another —
//! so what a name says is a set, [`Mentions`], and every check keeps its own
//! order of precedence over that set. What changed is only what counts as
//! saying "kitchen", not what a kitchen means to each of them.
//!
//! A room whose use was declared (see [`crate::RoomUse`]) is not read by its
//! name at all: the declaration says what it is, in no language.

use crate::annotations::fold;

/// One thing a room's name can say it is.
///
/// Finer than [`crate::RoomUse`], because the checks draw finer lines than
/// the room program does: a copa is a kitchen to NBR 5410 and not to the
/// lighting table, a lavabo is a bathroom that takes no shower, a study is an
/// office to the review and not to the socket count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Mention {
    Bedroom,
    Living,
    /// A TV room: a living room, to the checks that care.
    Tv,
    Dining,
    Kitchen,
    /// A copa: a small kitchen or breakfast room.
    Pantry,
    /// An "espaço gourmet": cooking for guests, often on a balcony.
    Gourmet,
    Laundry,
    Bathroom,
    /// A lavabo: a toilet and a basin, no shower.
    Lavatory,
    Office,
    Study,
    Corridor,
    Stairs,
    Entrance,
    /// A room for clothes.
    Closet,
    /// Storage nobody stays in: a store room, a larder, a linen room.
    Storage,
    /// A shaft or a plant room.
    Technical,
    HomeTheater,
    Balcony,
    /// Open to the sky: what falls on it is rain, not sewage.
    Uncovered,
    Garage,
    Outdoor,
}

/// Everything a name mentions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mentions(u32);

impl Mentions {
    /// A name that says nothing we can read.
    pub const NONE: Self = Self(0);

    #[must_use]
    pub const fn of(mention: Mention) -> Self {
        Self(1 << mention as u8)
    }

    #[must_use]
    pub const fn with(self, mention: Mention) -> Self {
        Self(self.0 | 1 << mention as u8)
    }

    #[must_use]
    pub const fn has(self, mention: Mention) -> bool {
        self.0 & (1 << mention as u8) != 0
    }

    /// Whether any of `these` is mentioned.
    #[must_use]
    pub fn any(self, these: &[Mention]) -> bool {
        these.iter().any(|&m| self.has(m))
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// A language we can read room names in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Portuguese,
    English,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::Portuguese, Self::English];

    /// The words of this language, folded (see [`fold`]), and what each
    /// means. A word is found anywhere in the name, so a stem stands for its
    /// family: `banh` is `banheiro`, `banho` and `banheira`.
    #[must_use]
    pub const fn words(self) -> &'static [(&'static str, Mention)] {
        match self {
            Self::Portuguese => PORTUGUESE,
            Self::English => ENGLISH,
        }
    }
}

use Mention::{
    Balcony, Bathroom, Bedroom, Closet, Corridor, Dining, Entrance, Garage, Gourmet, HomeTheater,
    Kitchen, Laundry, Lavatory, Living, Office, Outdoor, Pantry, Stairs, Storage, Study, Technical,
    Tv, Uncovered,
};

const PORTUGUESE: &[(&str, Mention)] = &[
    ("quarto", Bedroom),
    ("dormit", Bedroom),
    ("suite", Bedroom),
    ("sala", Living),
    ("estar", Living),
    ("tv", Tv),
    ("jantar", Dining),
    ("cozinha", Kitchen),
    ("copa", Pantry),
    ("gourmet", Gourmet),
    ("lavanderia", Laundry),
    ("servico", Laundry),
    ("banh", Bathroom),
    ("wc", Bathroom),
    ("sanitario", Bathroom),
    ("lavabo", Lavatory),
    ("escritorio", Office),
    ("home office", Office),
    ("estudo", Study),
    ("corredor", Corridor),
    ("circulacao", Corridor),
    ("hall", Corridor),
    ("passagem", Corridor),
    ("escada", Stairs),
    ("entrada", Entrance),
    ("closet", Closet),
    ("vestiario", Closet),
    ("vestidor", Closet),
    ("dressing", Closet),
    ("deposito", Storage),
    ("despensa", Storage),
    ("rouparia", Storage),
    ("shaft", Technical),
    ("area tecnica", Technical),
    ("home theater", HomeTheater),
    ("cinema", HomeTheater),
    ("varanda", Balcony),
    ("sacada", Balcony),
    ("terraco", Balcony),
    ("terraco", Uncovered),
    ("descobert", Uncovered),
    ("garage", Garage),
    ("quintal", Outdoor),
    ("jardim", Outdoor),
    ("externa", Outdoor),
    ("piscina", Outdoor),
    ("area de lazer", Outdoor),
    ("gramado", Outdoor),
    ("deck", Outdoor),
];

const ENGLISH: &[(&str, Mention)] = &[
    ("bed", Bedroom),
    ("nursery", Bedroom),
    ("living", Living),
    ("family room", Living),
    ("great room", Living),
    ("lounge", Living),
    ("dining", Dining),
    ("kitchen", Kitchen),
    ("laundry", Laundry),
    ("utility", Laundry),
    ("bath", Bathroom),
    ("restroom", Bathroom),
    ("toilet", Bathroom),
    ("powder room", Lavatory),
    ("office", Office),
    ("study", Study),
    ("corridor", Corridor),
    ("hallway", Corridor),
    ("stair", Stairs),
    ("entry", Entrance),
    ("entrance", Entrance),
    ("foyer", Entrance),
    ("walk-in", Closet),
    ("storage", Storage),
    ("pantry", Storage),
    ("linen", Storage),
    ("mechanical", Technical),
    ("media room", HomeTheater),
    ("home theatre", HomeTheater),
    ("balcony", Balcony),
    ("terrace", Balcony),
    ("terrace", Uncovered),
    ("uncovered", Uncovered),
    ("roof deck", Uncovered),
    ("porch", Balcony),
    ("yard", Outdoor),
    ("garden", Outdoor),
    ("pool", Outdoor),
    ("patio", Outdoor),
    ("outdoor", Outdoor),
    ("lawn", Outdoor),
];

/// What `name` mentions, in any language we hold.
#[must_use]
pub fn mentions(name: &str) -> Mentions {
    let name = fold(name);
    Language::ALL
        .iter()
        .flat_map(|language| language.words())
        .filter(|(word, _)| name.contains(word))
        .fold(Mentions::default(), |found, &(_, mention)| {
            found.with(mention)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_room_is_recognised_in_either_language() {
        for (pt, en, what) in [
            ("Quarto", "Bedroom", Bedroom),
            ("Suíte master", "Master bedroom", Bedroom),
            ("Cozinha", "Kitchen", Kitchen),
            ("Banheiro", "Bathroom", Bathroom),
            ("Lavabo", "Powder room", Lavatory),
            ("Sala de estar", "Living room", Living),
            ("Sala de jantar", "Dining room", Dining),
            ("Área de serviço", "Laundry", Laundry),
            ("Escritório", "Office", Office),
            ("Corredor", "Hallway", Corridor),
            ("Closet", "Walk-in closet", Closet),
            ("Varanda", "Balcony", Balcony),
            ("Garagem", "Garage", Garage),
            ("Jardim", "Garden", Outdoor),
        ] {
            assert!(mentions(pt).has(what), "{pt}");
            assert!(mentions(en).has(what), "{en}");
        }
    }

    #[test]
    fn a_name_can_say_more_than_one_thing() {
        let open = mentions("Sala e cozinha");
        assert!(open.has(Living) && open.has(Kitchen));
        // A bathroom of a suite is both, and each check decides which wins.
        let suite = mentions("Banho suíte");
        assert!(suite.has(Bathroom) && suite.has(Bedroom));
        assert!(mentions("Depósito").any(&[Storage, Technical]));
        assert!(mentions("Azul").is_empty());
    }

    #[test]
    fn every_word_is_already_folded() {
        // A word with an accent or a capital would never be found in a
        // folded name.
        for language in Language::ALL {
            for (word, _) in language.words() {
                assert_eq!(fold(word), *word, "{language:?}: {word}");
            }
        }
    }
}
