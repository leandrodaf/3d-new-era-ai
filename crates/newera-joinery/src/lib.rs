//! Parametric joinery and interiors.
//!
//! An agent (or the user) states intent with a few flat parameters — a
//! 200 cm wardrobe with two hinged doors and four shelves — and this crate
//! does the exact work: every board with its thickness, grooves, clearances
//! and hardware, checked against construction rules. Anything that cannot be
//! built comes back as a sentence that says what to change.
//!
//! Geometry is in centimeters, board thicknesses in millimeters as they are
//! sold. A build is a list of [`Part`]s in the piece's local frame (x across
//! the width from the left edge, y from the back to the front, z up from the
//! floor) that [`assemble`] turns into a furniture group, and [`cut_list`]
//! turns into rows for the workshop.

// Board counts and sizes in cm are small, far from integer or float limits.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

mod cabinet;
mod ceiling;
mod countertop;
mod cutlist;
mod slats;
mod sofa;

pub use cabinet::{CabinetParams, DoorType};
pub use ceiling::{CoveParams, CoveType, ShadowGapParams};
pub use countertop::{CountertopParams, Cutout, CutoutKind, Support};
pub use cutlist::{CutRow, cut_list, cut_list_csv, cut_list_dxf, cut_list_svg};
pub use slats::{Orientation, SlatsParams};
pub use sofa::{ArmType, SofaParams};

use newera_core::{Furniture, FurnitureId, Material, Point2, Properties};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Property keys stored on generated groups and parts.
pub const KIND_KEY: &str = "joinery:kind";
pub const PARAMS_KEY: &str = "joinery:params";
pub const PART_KEY: &str = "joinery:part";
pub const BOARD_KEY: &str = "joinery:board";
pub const EDGE_KEY: &str = "joinery:edge";

/// What to build, with its parameters. Every field has a sensible default,
/// so `{"kind":"cabinet","w":200}` is a complete request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Build {
    Cabinet(CabinetParams),
    Slats(SlatsParams),
    Countertop(CountertopParams),
    Cove(CoveParams),
    ShadowGap(ShadowGapParams),
    Sofa(SofaParams),
}

impl Build {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Cabinet(_) => "cabinet",
            Self::Slats(_) => "slats",
            Self::Countertop(_) => "countertop",
            Self::Cove(_) => "cove",
            Self::ShadowGap(_) => "shadow_gap",
            Self::Sofa(_) => "sofa",
        }
    }
}

/// One board, block or strip, as a box in the local frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    /// Workshop name, e.g. `Lateral esquerda`.
    pub name: String,
    /// Minimum corner, cm.
    pub at: [f64; 3],
    /// Size along x, y, z, cm.
    pub size: [f64; 3],
    /// Board material for the cut list (`MDF 18`), `None` for hardware-like
    /// or non-sheet parts (cushions, LED strips).
    pub board: Option<String>,
    /// Look in 3D.
    pub color: [u8; 3],
    pub finish: Option<Material>,
    /// Edges to band, as a count of long and short edges `[long, short]`.
    pub edge: [u8; 2],
    pub opacity: Option<f64>,
    /// Plan outline (local x, y) instead of a box: mitred plaster strips
    /// following a room. `at`/`size` still give its bounds and height.
    pub outline: Option<Vec<[f64; 2]>>,
    /// Length × width for the cut list when it isn't the box (mitred strips).
    pub cut: Option<[f64; 2]>,
}

impl Part {
    pub(crate) fn board(
        name: &str,
        at: [f64; 3],
        size: [f64; 3],
        board: &str,
        color: [u8; 3],
    ) -> Self {
        Self {
            name: name.to_owned(),
            at,
            size,
            board: Some(board.to_owned()),
            color,
            finish: None,
            edge: [0, 0],
            opacity: None,
            outline: None,
            cut: None,
        }
    }

    pub(crate) fn solid(name: &str, at: [f64; 3], size: [f64; 3], color: [u8; 3]) -> Self {
        Self {
            board: None,
            ..Self::board(name, at, size, "", color)
        }
    }

    pub(crate) fn banded(mut self, long: u8, short: u8) -> Self {
        self.edge = [long, short];
        self
    }
}

/// A finished build: its parts, overall size and what the workshop should know.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub parts: Vec<Part>,
    /// Overall width, depth, height, cm.
    pub size: [f64; 3],
    /// Hardware to buy, e.g. `8 dobradiças de caneco 35 mm`.
    pub hardware: Vec<String>,
    /// Things that work but deserve a look.
    pub notes: Vec<String>,
    /// Boards for the cut list that aren't drawn as parts (a whole stone top
    /// the 3D splits around its cutouts).
    pub extra_cuts: Vec<Part>,
    /// Default name of the group.
    pub name: String,
}

/// Generates the parts for `build`.
///
/// # Errors
/// A sentence describing what cannot be built and the value that would work.
pub fn generate(build: &Build) -> Result<Output, String> {
    match build {
        Build::Cabinet(p) => cabinet::generate(p),
        Build::Slats(p) => slats::generate(p),
        Build::Countertop(p) => countertop::generate(p),
        Build::Cove(p) => ceiling::cove(p),
        Build::ShadowGap(p) => ceiling::shadow_gap(p),
        Build::Sofa(p) => sofa::generate(p),
    }
}

/// Merges `patch` (new values for some parameters) into the parameters a
/// group was built with.
///
/// # Errors
/// When the stored or merged parameters don't describe a build.
pub fn merged(stored: &str, patch: &serde_json::Value) -> Result<Build, String> {
    let mut value: serde_json::Value = serde_json::from_str(stored)
        .map_err(|e| format!("stored parameters are unreadable: {e}"))?;
    if let (Some(target), Some(changes)) = (value.as_object_mut(), patch.as_object()) {
        for (key, v) in changes {
            if key != "kind" {
                target.insert(key.clone(), v.clone());
            }
        }
    }
    serde_json::from_value(value).map_err(|e| format!("invalid parameters: {e}"))
}

/// The build as a furniture group placed with its back-left-bottom corner
/// logic centered at `position`, turned `angle` degrees, raised `elevation`.
/// Pieces get ids from `next_id`.
pub fn assemble(
    build: &Build,
    output: &Output,
    group_id: FurnitureId,
    position: Point2,
    angle: f64,
    elevation: f64,
    next_id: &mut dyn FnMut() -> FurnitureId,
) -> Furniture {
    let [w, d, h] = output.size;
    let mut properties = Properties::new();
    properties.insert(KIND_KEY.into(), build.kind().into());
    properties.insert(
        PARAMS_KEY.into(),
        serde_json::to_string(build).unwrap_or_default(),
    );
    let mut group = Furniture {
        id: group_id,
        catalog: "group".into(),
        name: output.name.clone(),
        position,
        angle,
        elevation,
        width: w.max(0.1),
        depth: d.max(0.1),
        height: h.max(0.1),
        properties,
        ..Furniture::default()
    };
    group.children = output
        .parts
        .iter()
        .map(|part| {
            // Local frame: x from the left edge, y from the back; the group is
            // centered, with its front toward +y like every piece.
            let cx = part.at[0] + part.size[0] / 2.0 - w / 2.0;
            let cy = part.at[1] + part.size[1] / 2.0 - d / 2.0;
            let mut properties = Properties::new();
            properties.insert(PART_KEY.into(), part.name.clone());
            if let Some(board) = &part.board {
                properties.insert(BOARD_KEY.into(), board.clone());
            }
            if part.edge != [0, 0] {
                properties.insert(
                    EDGE_KEY.into(),
                    format!("{}+{}", part.edge[0], part.edge[1]),
                );
            }
            Furniture {
                id: next_id(),
                catalog: "box".into(),
                name: part.name.clone(),
                position: group.to_plan((cx, cy)),
                angle,
                elevation: elevation + part.at[2],
                width: part.size[0].max(0.05),
                depth: part.size[1].max(0.05),
                height: part.size[2].max(0.05),
                // A plain color finish is just the piece's color.
                color: Some(
                    part.finish
                        .as_ref()
                        .filter(|f| f.pattern.is_none() && f.image.is_none())
                        .and_then(|f| f.color)
                        .unwrap_or(part.color),
                ),
                texture: part
                    .finish
                    .clone()
                    .filter(|f| f.pattern.is_some() || f.image.is_some()),
                opacity: part.opacity,
                shape: part.outline.as_ref().map(|points| {
                    let center = [
                        part.at[0] + part.size[0] / 2.0,
                        part.at[1] + part.size[1] / 2.0,
                    ];
                    newera_core::SolidShape::Outline(
                        points
                            .iter()
                            .map(|p| [p[0] - center[0], p[1] - center[1]])
                            .collect(),
                    )
                }),
                properties,
                ..Furniture::default()
            }
        })
        .collect();
    group
}

/// Board thickness in cm from millimeters.
pub(crate) fn cm(mm: f64) -> f64 {
    mm / 10.0
}

/// A number written the Brazilian way in messages: `55`, `1,8`.
pub(crate) fn num(v: f64) -> String {
    let rounded = (v * 10.0).round() / 10.0;
    let text = if (rounded - rounded.round()).abs() < 1e-9 {
        format!("{}", rounded.round())
    } else {
        format!("{rounded:.1}")
    };
    text.replace('.', ",")
}

pub(crate) const MDF_WHITE: [u8; 3] = [238, 236, 230];

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    #[test]
    fn assembled_groups_place_parts_and_keep_their_parameters() {
        let build: Build = serde_json::from_str(
            r##"{"kind":"cabinet","w":100,"h":90,"d":50,"front":"#5f6e4a","shelves":1}"##,
        )
        .unwrap();
        let output = generate(&build).unwrap();
        let mut next_id = 10;
        let group = assemble(
            &build,
            &output,
            FurnitureId(1),
            Point2::new(500.0, 200.0),
            90.0,
            0.0,
            &mut || {
                next_id += 1;
                FurnitureId(next_id)
            },
        );
        assert_eq!(group.children.len(), output.parts.len());
        assert!(
            group
                .properties
                .get(PARAMS_KEY)
                .is_some_and(|p| p.contains("\"w\":100"))
        );
        // Turned 90°: the left side (local x −48,1) lands at plan y −48,1 from the center.
        let side = group
            .children
            .iter()
            .find(|c| c.name == "Lateral esquerda")
            .unwrap();
        assert!(
            (side.position.x - 500.0).abs() < 2.0
                && (side.position.y - (200.0 - 49.1)).abs() < 0.01,
            "{:?}",
            side.position
        );
        let door = group
            .children
            .iter()
            .find(|c| c.name.starts_with("Porta"))
            .unwrap();
        assert_eq!(door.color, Some([0x5f, 0x6e, 0x4a]));
        assert!(door.texture.is_none());
        assert_eq!(
            door.properties.get(BOARD_KEY).map(String::as_str),
            Some("MDF 18")
        );
    }

    #[test]
    fn builds_round_trip_and_merge_patches() {
        let build: Build = serde_json::from_str(r#"{"kind":"cabinet","w":120}"#).unwrap();
        let stored = serde_json::to_string(&build).unwrap();
        let patched = merged(&stored, &serde_json::json!({"shelves": 3, "kind": "sofa"})).unwrap();
        let Build::Cabinet(p) = patched else {
            panic!("kind never changes")
        };
        assert_eq!((p.w, p.shelves), (120.0, 3));
        assert!(merged(&stored, &serde_json::json!({"w": "wide"})).is_err());
        assert_eq!(num(1.8), "1,8");
        assert_eq!(num(55.0), "55");
    }
}
