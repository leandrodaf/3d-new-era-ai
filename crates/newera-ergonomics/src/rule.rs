//! What a finding is about, apart from how its sentence reads.
//!
//! A finding is accepted by key, and the key was built out of the first words
//! of its message. That made the prose load-bearing: rewording a rule — to say
//! it better, or to say it in another language — renamed the key and dropped
//! every acceptance already written in somebody's project, silently. The rule
//! that a finding answers to is not its wording, so it is named here, once,
//! and the wording is free to change.
//!
//! An id is written into saved projects the moment somebody accepts a finding.
//! It is data, not a label: **never rename one**. A rule that stops making
//! sense is removed and its acceptances go orphaned, which is visible; a rule
//! renamed in place takes its acceptances down without a word.

/// The rule behind a finding: the first part of [`crate::Finding::key`].
///
/// Every finding declares one, so a rule cannot be added without saying what
/// it is — the type is the check, not a test that has to remember.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Rule {
    // --- Who lives here, and what the home offers them ---
    SleepingPlaces,
    BedroomCrowding,
    BedroomSharing,
    NoBedroom,
    NoToilet,
    BathroomCount,
    DiningSeats,
    LivingSeats,
    WardrobeCapacity,
    // --- Getting in and through ---
    DoorClearWidth,
    DoorWidthWheelchair,
    RoomWithoutAccess,
    RoomWithoutDoor,
    DoorLeafBlocked,
    SamePlace,
    Clearance,
    // --- The room itself ---
    RoomArea,
    RoomNarrowSide,
    CeilingHeight,
    RoomWithoutWindow,
    WindowArea,
    MissingFixtures,
    WheelchairTurn,
    ElderlyBathroom,
    RoomUseConflict,
    BathroomFixture,
    UnusedCorner,
    // --- The kitchen at work ---
    WorkTriangleLong,
    WorkTriangleShort,
    SinkNextToStove,
    CounterHeight,
    WallCabinetLow,
    WallCabinetHigh,
    CounterTotal,
    CounterRunShort,
    KitchenSpread,
    MissingWorkZone,
    KitchenSockets,
    CounterSockets,
    KitchenWidth,
    FreeCircle,
    CityNotDeclared,
    // --- Gas, fumes and air ---
    GasVentPartial,
    GasWithoutVent,
    GasWindowNotVent,
    StoveWithoutHood,
    HoodNarrowerThanStove,
    GasHeaterInBathroom,
    GasCookingInBedroom,
    // --- Reach, sight and the gaps between pieces ---
    TvDistance,
    BetweenBeds,
    BasinHeightWheelchair,
    BedHeightWheelchair,
    SwitchOutletReach,
}

impl Rule {
    /// The id this rule is accepted by. Stable across editions, wordings and
    /// languages: it is written into saved projects.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::SleepingPlaces => "sleeping_places",
            Self::BedroomCrowding => "bedroom_crowding",
            Self::BedroomSharing => "bedroom_sharing",
            Self::NoBedroom => "no_bedroom",
            Self::NoToilet => "no_toilet",
            Self::BathroomCount => "bathroom_count",
            Self::DiningSeats => "dining_seats",
            Self::LivingSeats => "living_seats",
            Self::WardrobeCapacity => "wardrobe_capacity",
            Self::DoorClearWidth => "door_clear_width",
            Self::DoorWidthWheelchair => "door_width_wheelchair",
            Self::RoomWithoutAccess => "room_without_access",
            Self::RoomWithoutDoor => "room_without_door",
            Self::DoorLeafBlocked => "door_leaf_blocked",
            // Not `overlap`: that key space is check_layout's.
            Self::SamePlace => "same_place",
            Self::Clearance => "clearance",
            Self::RoomArea => "room_area",
            Self::RoomNarrowSide => "room_narrow_side",
            Self::CeilingHeight => "ceiling_height",
            Self::RoomWithoutWindow => "room_without_window",
            Self::WindowArea => "window_area",
            Self::MissingFixtures => "missing_fixtures",
            Self::WheelchairTurn => "wheelchair_turn",
            Self::ElderlyBathroom => "elderly_bathroom",
            // Written by this name since before rules were named: kept.
            Self::RoomUseConflict => "room_use",
            Self::BathroomFixture => "bathroom_fixture",
            Self::UnusedCorner => "unused_corner",
            Self::WorkTriangleLong => "work_triangle_long",
            Self::WorkTriangleShort => "work_triangle_short",
            Self::SinkNextToStove => "sink_next_to_stove",
            Self::CounterHeight => "counter_height",
            Self::WallCabinetLow => "wall_cabinet_low",
            Self::WallCabinetHigh => "wall_cabinet_high",
            Self::CounterTotal => "counter_total",
            Self::CounterRunShort => "counter_run_short",
            Self::KitchenSpread => "kitchen_spread",
            Self::MissingWorkZone => "missing_work_zone",
            Self::KitchenSockets => "kitchen_sockets",
            Self::CounterSockets => "counter_sockets",
            Self::KitchenWidth => "kitchen_width",
            Self::FreeCircle => "free_circle",
            Self::CityNotDeclared => "city_not_declared",
            Self::GasVentPartial => "gas_vent_partial",
            Self::GasWithoutVent => "gas_without_vent",
            Self::GasWindowNotVent => "gas_window_not_vent",
            Self::StoveWithoutHood => "stove_without_hood",
            Self::HoodNarrowerThanStove => "hood_narrower_than_stove",
            Self::GasHeaterInBathroom => "gas_heater_in_bathroom",
            Self::GasCookingInBedroom => "gas_cooking_in_bedroom",
            Self::TvDistance => "tv_distance",
            Self::BetweenBeds => "between_beds",
            Self::BasinHeightWheelchair => "basin_height_wheelchair",
            Self::BedHeightWheelchair => "bed_height_wheelchair",
            Self::SwitchOutletReach => "switch_outlet_reach",
        }
    }

    /// Every rule there is, for the tests that hold the ids to their word.
    #[cfg(test)]
    pub(crate) const ALL: &'static [Self] = &[
        Self::SleepingPlaces,
        Self::BedroomCrowding,
        Self::BedroomSharing,
        Self::NoBedroom,
        Self::NoToilet,
        Self::BathroomCount,
        Self::DiningSeats,
        Self::LivingSeats,
        Self::WardrobeCapacity,
        Self::DoorClearWidth,
        Self::DoorWidthWheelchair,
        Self::RoomWithoutAccess,
        Self::RoomWithoutDoor,
        Self::DoorLeafBlocked,
        Self::SamePlace,
        Self::Clearance,
        Self::RoomArea,
        Self::RoomNarrowSide,
        Self::CeilingHeight,
        Self::RoomWithoutWindow,
        Self::WindowArea,
        Self::MissingFixtures,
        Self::WheelchairTurn,
        Self::ElderlyBathroom,
        Self::RoomUseConflict,
        Self::BathroomFixture,
        Self::UnusedCorner,
        Self::WorkTriangleLong,
        Self::WorkTriangleShort,
        Self::SinkNextToStove,
        Self::CounterHeight,
        Self::WallCabinetLow,
        Self::WallCabinetHigh,
        Self::CounterTotal,
        Self::CounterRunShort,
        Self::KitchenSpread,
        Self::MissingWorkZone,
        Self::KitchenSockets,
        Self::CounterSockets,
        Self::KitchenWidth,
        Self::FreeCircle,
        Self::CityNotDeclared,
        Self::GasVentPartial,
        Self::GasWithoutVent,
        Self::GasWindowNotVent,
        Self::StoveWithoutHood,
        Self::HoodNarrowerThanStove,
        Self::GasHeaterInBathroom,
        Self::GasCookingInBedroom,
        Self::TvDistance,
        Self::BetweenBeds,
        Self::BasinHeightWheelchair,
        Self::BedHeightWheelchair,
        Self::SwitchOutletReach,
    ];
}

/// The key a finding of this rule about this place is accepted by.
///
/// `place` is the label a finding carries (`Quarto r5`, `Cama de casal f12`),
/// and what identifies it is the id at the end, not the name in front: a room
/// keeps its acceptances when it is renamed.
pub(crate) fn key(rule: Rule, place: &str) -> String {
    format!("{}:{}", rule.id(), place_id(place))
}

/// The same, for a rule that speaks more than once about one place: the gap
/// between two beds of a bedroom is one finding per pair, not per room.
pub(crate) fn key_at(rule: Rule, place: &str, about: &str) -> String {
    format!("{}:{}:{about}", rule.id(), place_id(place))
}

/// The id at the end of a label, folded: `Quarto r5` is `r5`.
fn place_id(place: &str) -> String {
    newera_core::fold(place)
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{Rule, key, key_at};

    #[test]
    fn every_rule_has_its_own_id_and_none_reads_as_another_key() {
        let mut seen = std::collections::BTreeMap::new();
        for &rule in Rule::ALL {
            let id = rule.id();
            assert!(
                !id.is_empty()
                    && id
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()),
                "{rule:?}: an id travels in a key, so it is lowercase ascii: {id}"
            );
            // The disciplines and the layout check own these prefixes; a rule
            // borrowing one would be sorted, accepted and orphaned as theirs.
            for taken in ["elec", "plumb", "overlap", "outside", "above"] {
                assert_ne!(id, taken, "{rule:?}: {taken} belongs to another check");
            }
            if let Some(before) = seen.insert(id, rule) {
                panic!("{rule:?} and {before:?} share the id {id}");
            }
        }
    }

    #[test]
    fn a_key_names_the_rule_and_the_id_of_the_place_not_its_name() {
        assert_eq!(key(Rule::CeilingHeight, "Quarto r5"), "ceiling_height:r5");
        assert_eq!(
            key(Rule::CeilingHeight, "Quarto de hóspedes renomeado r5"),
            "ceiling_height:r5",
            "renaming a room keeps what its acceptances are written under"
        );
        assert_eq!(key(Rule::SleepingPlaces, "Casa"), "sleeping_places:casa");
        assert_eq!(
            key_at(Rule::BetweenBeds, "Quarto r5", "f20+f21"),
            "between_beds:r5:f20+f21"
        );
    }
}
